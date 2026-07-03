//! HTTP + WebSocket API handlers over `wipe-core`.

use std::path::PathBuf;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use wipe_core::model::{Board, IdentityKind, Ticket};
use wipe_core::ops::{self, NewTicket, TicketPatch};
use wipe_core::{git, Store};

/// Shared server state.
#[derive(Clone)]
pub struct AppState {
    /// The project the daemon was started in (default target).
    pub current: PathBuf,
    /// Broadcast channel for live-update notifications.
    pub tx: broadcast::Sender<String>,
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

fn store_for(state: &AppState, project: Option<String>) -> Result<Store, ApiError> {
    let root = project
        .map(PathBuf::from)
        .unwrap_or_else(|| state.current.clone());
    Ok(Store::open(root)?)
}

fn notify(state: &AppState) {
    let _ = state.tx.send("changed".to_string());
}

/// Who to attribute a UI-driven mutation to for the activity timeline: an explicit
/// `actor` from the request if given, else the repo's configured git identity, else
/// a generic fallback.
fn resolve_actor(store: &Store, provided: Option<String>) -> String {
    provided
        .filter(|s| !s.trim().is_empty())
        .or_else(|| git::config_identity(store.root()))
        .unwrap_or_else(|| "someone".to_string())
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

/// `GET /api/projects`
pub async fn projects(State(state): State<AppState>) -> ApiResult {
    crate::registry::register(&state.current);
    Ok(Json(json!({ "projects": crate::registry::list() })))
}

/// `GET /api/board`
pub async fn board(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let (board, view) = ops::board_view(&store)?;
    Ok(Json(board_json(&board, &view)))
}

/// `GET /api/history` — commits touching `.wipe/`, most recent first.
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

/// `GET /api/board/at` — reconstruct the board as of a commit (the rewind feature).
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
    Json(b): Json<CreateTicketBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
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
    Json(b): Json<MoveBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
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
    Json(b): Json<CommentBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
    let author = b.author.unwrap_or_else(|| "ui".to_string());
    let cid = ops::add_comment(&store, &id, &author, &b.body, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "ticket": id, "comment": cid })))
}

/// `GET /api/definitions` — labels + priorities.
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

/// `POST /api/labels` — define a new label (auto-colored if no color given).
pub async fn create_label(State(state): State<AppState>, Json(b): Json<LabelBody>) -> ApiResult {
    let store = store_for(&state, b.project)?;
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

/// `PATCH /api/labels/{name}` — change a label's color.
pub async fn recolor_label(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(b): Json<LabelColorBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
    let label = ops::set_label_color(&store, &name, &b.color)?;
    notify(&state);
    Ok(Json(serde_json::to_value(label)?))
}

/// `DELETE /api/labels/{name}` — delete a label and strip it from all tickets.
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

/// `PATCH /api/tickets/{id}` — update ticket fields.
pub async fn patch_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<PatchBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
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

/// `GET /api/identities` — humans (from git) + agents (registry).
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

/// `PUT /api/identities/{id}` — set an identity's display name / kind.
pub async fn put_identity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<IdentityBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
    let kind = match b.kind.as_deref() {
        Some("agent") => Some(IdentityKind::Agent),
        Some("human") => Some(IdentityKind::Human),
        _ => None,
    };
    let ident = ops::upsert_identity(&store, &id, &b.display_name, kind)?;
    notify(&state);
    Ok(Json(serde_json::to_value(ident)?))
}

/// `DELETE /api/identities/{id}` — remove an identity from the registry (agents /
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

/// `POST /api/tickets/{id}/attachments` — multipart file upload (field `file`).
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

/// `DELETE /api/tickets/{id}/attachments` — detach by repo-relative path.
pub async fn delete_attachment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<DetachBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
    let actor = resolve_actor(&store, b.actor);
    ops::remove_attachment(&store, &id, &b.path, &actor, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/media/{*path}` — serve an attachment for preview/download.
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

/// `GET /api/graph` — the commit graph (all branches) with board checkpoints.
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

/// `POST /api/lists` — add a list to the board.
pub async fn add_list(State(state): State<AppState>, Json(b): Json<AddListBody>) -> ApiResult {
    let store = store_for(&state, b.project)?;
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

/// `PATCH /api/lists/{id}` — rename a list.
pub async fn rename_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<RenameListBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
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

/// `POST /api/lists/{id}/move` — reorder a list to a new index.
pub async fn move_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<MoveListBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
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

/// `DELETE /api/lists/{id}` — remove a list (use `?force=true` to delete its cards).
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

// --- websocket -------------------------------------------------------------

/// `GET /ws` — upgrade to a WebSocket that streams change notifications.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| ws_loop(socket, state))
}

async fn ws_loop(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
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
