//! HTTP + WebSocket API handlers over `wipe-core`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use wipe_core::forum::{self, NewReply, NewThread, SearchQuery};
use wipe_core::model::{Board, IdentityKind, Ticket};
use wipe_core::ops::{self, NewTicket, TicketPatch};
use wipe_core::{git, Store};

/// Shared server state.
#[derive(Clone)]
pub struct AppState {
    /// The project the daemon was started in, if any. Used only as the default
    /// target when a request omits `?project=`; every UI request names its project
    /// explicitly, so this is just a convenience for CLI-less callers. `None` when
    /// `wipe serve` runs outside a board (a purely global viewer).
    pub current: Option<PathBuf>,
    /// Broadcast channel for live-update notifications.
    pub tx: broadcast::Sender<String>,
    /// Number of live UI WebSocket clients. Drives idle-shutdown: when this is 0
    /// for long enough, an auto-served daemon exits.
    pub clients: Arc<AtomicUsize>,
}

/// An error that renders as a JSON `{ok:false,error}` body.
pub struct ApiError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "ok": false, "error": self.0.to_string() }));
        (StatusCode::BAD_REQUEST, body).into_response()
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

/// Query string carrying an optional project root.
#[derive(Debug, Deserialize)]
pub struct ProjectQuery {
    project: Option<String>,
}

/// Resolve which board a request targets. Prefer the explicit `project` (every UI
/// request sends one); fall back to the daemon's launch project. Erroring when
/// neither is available keeps a stray request from silently hitting the wrong board.
fn store_for(state: &AppState, project: Option<String>) -> Result<Store, ApiError> {
    let root = project
        .map(PathBuf::from)
        .or_else(|| state.current.clone())
        .ok_or_else(|| ApiError(anyhow::anyhow!("no project selected")))?;
    Ok(Store::open(root)?)
}

fn notify(state: &AppState) {
    let _ = state.tx.send("changed".to_string());
}

/// Who to attribute a UI-driven mutation to for the activity timeline: an explicit
/// `actor` from the request if given, else the repo's VCS user (git, Plastic, …),
/// the board default, or the configured global default - never "unknown".
fn resolve_actor(store: &Store, provided: Option<String>) -> String {
    ops::resolve_identity(Some(store), provided.as_deref())
}

fn board_json(board: &Board, view: &[(String, Vec<Ticket>)]) -> Value {
    let lists: Vec<Value> = view
        .iter()
        .map(|(list_id, tickets)| {
            let name = board
                .list(list_id)
                .map(|l| l.name.clone())
                .unwrap_or_else(|| list_id.clone());
            json!({ "list": list_id, "name": name, "tickets": tickets })
        })
        .collect();
    json!({ "board": board.name, "lists": lists })
}

// --- read endpoints --------------------------------------------------------

/// `GET /api/health`
pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "service": "wipe-daemon", "version": env!("CARGO_PKG_VERSION") }))
}

/// `GET /api/config` - user-global preferences (styling + default identity) so the
/// board UI can honor the choices made via `wipe config --global` / `wipe onboard`.
pub async fn app_config() -> Json<Value> {
    let g = wipe_core::GlobalConfig::load();
    Json(json!({
        "accent": g.ui_accent,
        "theme": g.ui_theme,
        "default_identity": g.default_identity,
        "prefer_default_identity": g.prefer_default_identity.unwrap_or(false),
    }))
}

/// Body for updating user-global preferences. Absent fields are left unchanged.
#[derive(Debug, Deserialize)]
pub struct ConfigPatch {
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    default_identity: Option<String>,
    #[serde(default)]
    prefer_default_identity: Option<bool>,
}

/// `PATCH /api/config` - update user-global preferences from the UI.
pub async fn patch_config(Json(b): Json<ConfigPatch>) -> ApiResult {
    let mut g = wipe_core::GlobalConfig::load();
    if let Some(a) = b.accent {
        g.ui_accent = Some(a);
    }
    if let Some(t) = b.theme {
        g.ui_theme = Some(t);
    }
    if let Some(id) = b.default_identity {
        let id = id.trim().to_string();
        if !id.is_empty() {
            g.default_identity = Some(id);
        }
    }
    if let Some(p) = b.prefer_default_identity {
        g.prefer_default_identity = Some(p);
    }
    g.save()
        .map_err(|e| ApiError(anyhow::anyhow!("saving config: {e}")))?;
    Ok(Json(json!({
        "ok": true,
        "accent": g.ui_accent,
        "theme": g.ui_theme,
        "default_identity": g.default_identity,
        "prefer_default_identity": g.prefer_default_identity.unwrap_or(false),
    })))
}

/// `POST /api/scan` - discover boards on disk and refresh the registry, returning
/// the updated project list (so the UI can offer a "rescan" action).
pub async fn rescan() -> ApiResult {
    // A blocking full-disk walk - keep it off the async worker threads.
    let (found, projects) = tokio::task::spawn_blocking(|| {
        crate::registry::prune();
        let roots: Vec<std::path::PathBuf> = {
            let g = wipe_core::GlobalConfig::load();
            match g.scan_roots {
                Some(r) if !r.is_empty() => r.into_iter().map(std::path::PathBuf::from).collect(),
                _ => crate::registry::default_scan_roots(),
            }
        };
        let found = crate::registry::scan(&roots, 7).len();
        (found, crate::registry::list())
    })
    .await
    .map_err(|e| ApiError(anyhow::anyhow!("scan task failed: {e}")))?;
    Ok(Json(json!({ "found": found, "projects": projects })))
}

/// `GET /api/projects`
///
/// Reports every registered board plus `current` - the project the daemon was
/// launched in - so the UI can default-open that board rather than an arbitrary
/// one. `current` is null when `wipe serve` was started outside any board.
pub async fn projects(State(state): State<AppState>) -> ApiResult {
    let current = state
        .current
        .as_ref()
        .filter(|root| Store::open(root).is_ok())
        .map(|root| root.display().to_string());
    if let Some(root) = &state.current {
        crate::registry::register(root);
    }
    Ok(Json(
        json!({ "projects": crate::registry::list(), "current": current }),
    ))
}

/// `GET /api/board`
pub async fn board(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let (board, view) = ops::board_view(&store)?;
    Ok(Json(board_json(&board, &view)))
}

/// `GET /api/history` - commits touching `.wipe/`, most recent first.
pub async fn history(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let commits = git::log(store.root(), Some(".wipe"), Some(200))?;
    Ok(Json(json!({ "commits": commits })))
}

/// Query for a historical board snapshot.
#[derive(Debug, Deserialize)]
pub struct AtQuery {
    project: Option<String>,
    commit: String,
}

/// `GET /api/board/at` - reconstruct the board as of a commit (the rewind feature).
pub async fn board_at(State(state): State<AppState>, Query(q): Query<AtQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let root = store.root();
    let board_src = git::file_at_commit(root, &q.commit, ".wipe/board.json")?
        .ok_or_else(|| ApiError(anyhow::anyhow!("no board at commit {}", q.commit)))?;
    let board: Board = serde_json::from_str(&board_src)?;

    let mut lists = Vec::with_capacity(board.lists.len());
    for list in &board.lists {
        let mut tickets = Vec::with_capacity(list.cards.len());
        for id in &list.cards {
            let rel = format!(".wipe/tickets/{id}.json");
            if let Some(src) = git::file_at_commit(root, &q.commit, &rel)? {
                if let Ok(t) = serde_json::from_str::<Ticket>(&src) {
                    tickets.push(t);
                }
            }
        }
        lists.push(json!({ "list": list.id, "name": list.name, "tickets": tickets }));
    }
    Ok(Json(
        json!({ "board": board.name, "commit": q.commit, "lists": lists }),
    ))
}

// --- write endpoints -------------------------------------------------------

/// Body for creating a ticket.
#[derive(Debug, Deserialize)]
pub struct CreateTicketBody {
    project: Option<String>,
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    list: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    assignees: Vec<String>,
    #[serde(default)]
    actor: Option<String>,
}

/// `POST /api/tickets`
pub async fn create_ticket(
    State(state): State<AppState>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<CreateTicketBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let actor = resolve_actor(&store, b.actor);
    let spec = NewTicket {
        title: b.title,
        body: b.body,
        priority: b.priority,
        list: b.list,
        labels: b.labels,
        assignees: b.assignees,
    };
    let ticket = ops::create_ticket(&store, spec, &actor, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(ticket)?))
}

/// Body for moving a ticket.
#[derive(Debug, Deserialize)]
pub struct MoveBody {
    project: Option<String>,
    to: String,
    #[serde(default)]
    pos: Option<usize>,
    #[serde(default)]
    actor: Option<String>,
}

/// `POST /api/tickets/{id}/move`
pub async fn move_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<MoveBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let actor = resolve_actor(&store, b.actor);
    ops::move_ticket(&store, &id, &b.to, b.pos, &actor, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id, "list": b.to })))
}

/// Body for adding a comment.
#[derive(Debug, Deserialize)]
pub struct CommentBody {
    project: Option<String>,
    #[serde(default)]
    author: Option<String>,
    body: String,
}

/// `POST /api/tickets/{id}/comments`
pub async fn add_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<CommentBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let author = b.author.unwrap_or_else(|| "ui".to_string());
    let cid = ops::add_comment(&store, &id, &author, &b.body, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "ticket": id, "comment": cid })))
}

// --- checklist ---------------------------------------------------------------

/// Body for adding a checklist item.
#[derive(Debug, Deserialize)]
pub struct ChecklistAddBody {
    project: Option<String>,
    text: String,
}

/// `POST /api/tickets/{id}/checklist` - append an item; returns the updated ticket.
pub async fn add_checklist_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistAddBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    ops::checklist_add(&store, &id, &b.text, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// Body for editing / (un)checking a checklist item. `done` sets the checked
/// state; `text` renames. Either or both may be present.
#[derive(Debug, Deserialize)]
pub struct ChecklistPatchBody {
    project: Option<String>,
    #[serde(default)]
    done: Option<bool>,
    #[serde(default)]
    text: Option<String>,
}

/// `PATCH /api/tickets/{id}/checklist/{item}` - set state and/or text; returns the
/// updated ticket.
pub async fn patch_checklist_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistPatchBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let now = Utc::now();
    if let Some(text) = b.text {
        ops::checklist_edit(&store, &id, &item, &text, now)?;
    }
    if let Some(done) = b.done {
        ops::checklist_set(&store, &id, &item, Some(done), now)?;
    }
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// `DELETE /api/tickets/{id}/checklist/{item}` - remove an item; returns the ticket.
/// The project is taken from the `?project=` query (no request body needed).
pub async fn delete_checklist_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    ops::checklist_remove(&store, &id, &item, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// Body for reordering a checklist item.
#[derive(Debug, Deserialize)]
pub struct ChecklistMoveBody {
    project: Option<String>,
    index: usize,
}

/// `POST /api/tickets/{id}/checklist/{item}/move` - reorder; returns the ticket.
pub async fn move_checklist_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistMoveBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    ops::checklist_move(&store, &id, &item, b.index, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// `GET /api/definitions` - labels + priorities.
pub async fn definitions(
    State(state): State<AppState>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    Ok(Json(serde_json::to_value(store.load_definitions()?)?))
}

/// Body for creating a label definition. `color` is optional (auto-assigned).
#[derive(Debug, Deserialize)]
pub struct LabelBody {
    project: Option<String>,
    name: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// `POST /api/labels` - define a new label (auto-colored if no color given).
pub async fn create_label(
    State(state): State<AppState>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<LabelBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let label = ops::create_label(&store, &b.name, b.color, b.description)?;
    notify(&state);
    Ok(Json(serde_json::to_value(label)?))
}

/// Body for updating a label's color.
#[derive(Debug, Deserialize)]
pub struct LabelColorBody {
    project: Option<String>,
    color: String,
}

/// `PATCH /api/labels/{name}` - change a label's color.
pub async fn recolor_label(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<LabelColorBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let label = ops::set_label_color(&store, &name, &b.color)?;
    notify(&state);
    Ok(Json(serde_json::to_value(label)?))
}

/// `DELETE /api/labels/{name}` - delete a label and strip it from all tickets.
pub async fn delete_label(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    ops::delete_label(&store, &name, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true })))
}

/// Body for patching a ticket. Absent fields are left unchanged; an explicit
/// `null` for `priority` clears it.
#[derive(Debug, Deserialize)]
pub struct PatchBody {
    project: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    priority: Option<Option<String>>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    assignees: Option<Vec<String>>,
    #[serde(default)]
    actor: Option<String>,
}

/// `PATCH /api/tickets/{id}` - update ticket fields.
pub async fn patch_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<PatchBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let actor = resolve_actor(&store, b.actor);
    let patch = TicketPatch {
        title: b.title,
        body: b.body,
        priority: b.priority,
        labels: b.labels,
        assignees: b.assignees,
    };
    let ticket = ops::update_ticket(&store, &id, patch, &actor, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(ticket)?))
}

/// `GET /api/identities` - humans (from git) + agents (registry).
pub async fn identities(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    Ok(Json(json!({ "identities": ops::list_identities(&store)? })))
}

/// Body for creating/updating an identity.
#[derive(Debug, Deserialize)]
pub struct IdentityBody {
    project: Option<String>,
    display_name: String,
    #[serde(default)]
    kind: Option<String>,
}

/// `PUT /api/identities/{id}` - set an identity's display name / kind.
pub async fn put_identity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<IdentityBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let kind = match b.kind.as_deref() {
        Some("agent") => Some(IdentityKind::Agent),
        Some("human") => Some(IdentityKind::Human),
        _ => None,
    };
    let ident = ops::upsert_identity(&store, &id, &b.display_name, kind)?;
    notify(&state);
    Ok(Json(serde_json::to_value(ident)?))
}

/// `DELETE /api/identities/{id}` - remove an identity from the registry (agents /
/// manual overrides; git-discovered humans reappear from history).
pub async fn delete_identity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    ops::delete_identity(&store, &id)?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id })))
}

/// `POST /api/tickets/{id}/attachments` - multipart file upload (field `file`).
pub async fn upload_attachment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ProjectQuery>,
    mut multipart: Multipart,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let actor = resolve_actor(&store, None);
    let max = store.load_settings()?.max_attachment_mb * 1024 * 1024;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(anyhow::anyhow!("bad upload: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().unwrap_or("file").to_string();
        let mime = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let bytes = field
            .bytes()
            .await
            .map_err(|e| ApiError(anyhow::anyhow!("read upload: {e}")))?;
        if bytes.len() as u64 > max {
            return Err(ApiError(anyhow::anyhow!(
                "attachment is {:.1} MB, over the {} MB limit",
                bytes.len() as f64 / 1_048_576.0,
                max / 1024 / 1024
            )));
        }
        let att = ops::add_attachment(&store, &id, &name, &bytes, &mime, &actor, Utc::now())?;
        notify(&state);
        return Ok(Json(serde_json::to_value(att)?));
    }
    Err(ApiError(anyhow::anyhow!("no `file` field in upload")))
}

/// Body for detaching an attachment.
#[derive(Debug, Deserialize)]
pub struct DetachBody {
    project: Option<String>,
    path: String,
    #[serde(default)]
    actor: Option<String>,
}

/// `DELETE /api/tickets/{id}/attachments` - detach by repo-relative path.
pub async fn delete_attachment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<DetachBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let actor = resolve_actor(&store, b.actor);
    ops::remove_attachment(&store, &id, &b.path, &actor, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/media/{*path}` - serve an attachment for preview/download.
pub async fn serve_media(
    State(state): State<AppState>,
    Query(q): Query<ProjectQuery>,
    Path(path): Path<String>,
) -> Response {
    let store = match store_for(&state, q.project) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let rel = path.replace('\\', "/");
    if rel.contains("..") {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    }
    let full = store.root().join(&rel);
    // Confine reads to within the project root.
    let within = std::fs::canonicalize(store.root())
        .ok()
        .zip(std::fs::canonicalize(&full).ok())
        .map(|(root, target)| target.starts_with(root))
        .unwrap_or(false);
    if !within {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    match std::fs::read(&full) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&full).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// `GET /api/graph` - the commit graph (all branches) with board checkpoints.
pub async fn graph(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let commits = git::graph(store.root(), Some(300))?;
    Ok(Json(json!({ "commits": commits })))
}

/// Body for adding a list.
#[derive(Debug, Deserialize)]
pub struct AddListBody {
    project: Option<String>,
    name: String,
}

/// `POST /api/lists` - add a list to the board.
pub async fn add_list(
    State(state): State<AppState>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<AddListBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let id = ops::add_list(&store, &b.name, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id, "name": b.name })))
}

/// Body for renaming a list.
#[derive(Debug, Deserialize)]
pub struct RenameListBody {
    project: Option<String>,
    name: String,
}

/// `PATCH /api/lists/{id}` - rename a list.
pub async fn rename_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<RenameListBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    ops::rename_list(&store, &id, &b.name, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id, "name": b.name })))
}

/// Body for reordering a list.
#[derive(Debug, Deserialize)]
pub struct MoveListBody {
    project: Option<String>,
    index: usize,
}

/// `POST /api/lists/{id}/move` - reorder a list to a new index.
pub async fn move_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<MoveListBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    ops::move_list(&store, &id, b.index, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id, "index": b.index })))
}

/// Query for removing a list.
#[derive(Debug, Deserialize)]
pub struct RemoveListQuery {
    project: Option<String>,
    #[serde(default)]
    force: bool,
}

/// `DELETE /api/lists/{id}` - remove a list (use `?force=true` to delete its cards).
pub async fn remove_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<RemoveListQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    ops::remove_list(&store, &id, q.force, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id })))
}

// --- forum -----------------------------------------------------------------

/// `GET /api/forum` - thread summaries (root posts + total post counts), newest first.
pub async fn forum_list(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let all = forum::index(&store)?;
    let threads: Vec<Value> = all
        .iter()
        .filter(|p| p.depth == 0)
        .map(|r| {
            let posts = all.iter().filter(|p| p.thread_id == r.thread_id).count();
            json!({
                "id": r.thread_id,
                "title": r.thread_title,
                "author": r.author,
                "labels": r.labels,
                "posts": posts,
                "created": r.created,
            })
        })
        .collect();
    Ok(Json(json!({ "threads": threads })))
}

/// `GET /api/forum/{id}` - a whole thread (root + nested reply tree).
pub async fn forum_thread(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    Ok(Json(serde_json::to_value(forum::get_thread(&store, &id)?)?))
}

/// Body for creating a thread.
#[derive(Debug, Deserialize)]
pub struct ForumPostBody {
    project: Option<String>,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    refs: Vec<String>,
    #[serde(default)]
    actor: Option<String>,
}

/// `POST /api/forum` - open a new thread.
pub async fn forum_create(
    State(state): State<AppState>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ForumPostBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let actor = resolve_actor(&store, b.actor);
    let t = forum::create_thread(
        &store,
        NewThread {
            title: b.title,
            body: b.body,
            labels: b.labels,
            refs: b.refs,
            attachments: Vec::new(),
        },
        &actor,
        Utc::now(),
    )?;
    notify(&state);
    Ok(Json(serde_json::to_value(t)?))
}

/// Body for replying to a post.
#[derive(Debug, Deserialize)]
pub struct ForumReplyBody {
    project: Option<String>,
    body: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    refs: Vec<String>,
    #[serde(default)]
    actor: Option<String>,
}

/// `POST /api/forum/{id}/reply` - reply to a post at any depth.
pub async fn forum_reply(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ForumReplyBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    let actor = resolve_actor(&store, b.actor);
    let child = forum::reply(
        &store,
        &id,
        NewReply {
            body: b.body,
            labels: b.labels,
            refs: b.refs,
            attachments: Vec::new(),
        },
        &actor,
        Utc::now(),
    )?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": child, "parent": id })))
}

/// Body for editing a post.
#[derive(Debug, Deserialize)]
pub struct ForumEditBody {
    project: Option<String>,
    body: String,
}

/// `PATCH /api/forum/{id}` - edit a post's body.
pub async fn forum_edit(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ForumEditBody>,
) -> ApiResult {
    let store = store_for(&state, pq.project.or(b.project))?;
    forum::edit_post(&store, &id, &b.body, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id })))
}

/// `DELETE /api/forum/{id}` - delete a post and its subtree.
pub async fn forum_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    forum::delete_post(&store, &id, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id })))
}

/// Query for a forum search.
#[derive(Debug, Deserialize)]
pub struct ForumSearchParams {
    project: Option<String>,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    titles: Option<bool>,
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

/// `GET /api/forum/search` - regex + filter search over posts.
pub async fn forum_search(
    State(state): State<AppState>,
    Query(p): Query<ForumSearchParams>,
) -> ApiResult {
    let store = store_for(&state, p.project)?;
    let pattern = match p.q.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(forum::compile_pattern(s, true)?),
        _ => None,
    };
    let query = SearchQuery {
        pattern,
        author: p.author,
        labels: p.label.into_iter().collect(),
        scope: p.scope,
        max_depth: p.depth,
        titles_only: p.titles.unwrap_or(false),
        limit: p.limit,
    };
    Ok(Json(json!({ "posts": forum::search(&store, &query)? })))
}

// --- websocket -------------------------------------------------------------

/// `GET /ws` - upgrade to a WebSocket that streams change notifications.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| ws_loop(socket, state))
}

/// Decrements the live-client counter when a WebSocket handler ends.
struct ClientGuard(Arc<AtomicUsize>);
impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn ws_loop(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    // Count this client for the lifetime of the socket so idle-shutdown knows the
    // board is actively being viewed; `_guard` decrements on drop (any exit path).
    state.clients.fetch_add(1, Ordering::SeqCst);
    let _guard = ClientGuard(state.clients.clone());
    let _ = socket.send(Message::Text("connected".into())).await;
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(m) => {
                    if socket.send(Message::Text(m.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => {}
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
}
