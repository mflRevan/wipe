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
    /// Bearer token required on `/api` + `/ws` when the daemon is exposed beyond
    /// localhost. `None` for localhost-only serves (no auth needed).
    pub token: Option<String>,
    /// Whether the daemon is reachable beyond localhost. When true, client-supplied
    /// `actor`/`author` from *remote* clients is ignored (a shared token can't verify
    /// per-user identity), so their writes are attributed to the board's own
    /// resolved identity instead.
    pub exposed: bool,
    /// Whether requests from this machine (loopback) skip the token and may name
    /// their identity. False behind a reverse proxy, where every request arrives
    /// via loopback.
    pub trust_loopback: bool,
    /// Per-project cache of the serialized `/api/board` payload, keyed by a cheap
    /// fingerprint of the board's files, so the UI's frequent polls neither
    /// re-read every ticket nor re-transfer an unchanged board.
    pub board_cache: Arc<std::sync::Mutex<std::collections::HashMap<PathBuf, BoardCache>>>,
    /// The URLs this daemon serves (set once bound), for the `/connect` page.
    pub connect_urls: Arc<std::sync::OnceLock<Vec<crate::net::ShownUrl>>>,
}

/// `GET /connect` - a page for opening the board on another device: every network
/// URL (with the access token) as a link and a scannable QR code. Shown only to
/// this machine - it hands out the token.
pub async fn connect_page(State(state): State<AppState>) -> Response {
    let local = TRUSTED.try_with(|t| *t).unwrap_or(false);
    if !local && state.exposed {
        return (
            StatusCode::FORBIDDEN,
            "open this page on the machine running wipe",
        )
            .into_response();
    }
    let urls = state.connect_urls.get().cloned().unwrap_or_default();
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let mut cards = String::new();
    for u in urls.iter().skip(1) {
        let svg = qrcode::QrCode::new(u.url.as_bytes())
            .map(|c| {
                c.render::<qrcode::render::svg::Color>()
                    .min_dimensions(220, 220)
                    .quiet_zone(true)
                    .build()
            })
            .unwrap_or_default();
        cards.push_str(&format!(
            "<section><h2>{}</h2><div class=qr>{svg}</div><a href=\"{}\">{}</a></section>",
            esc(&u.label),
            esc(&u.url),
            esc(&u.url)
        ));
    }
    if cards.is_empty() {
        cards = "<p class=none>This daemon only listens on this machine. Start it with <code>wipe serve</code>                  (local network) or <code>wipe serve --tailscale</code> to open the board from your phone.</p>"
            .to_string();
    }
    let html = format!(
        "<!doctype html><html><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">         <title>wipe - open on another device</title><style>         body{{font:15px system-ui,sans-serif;background:#1c1b1a;color:#eee;margin:0;padding:32px;}}         h1{{font-size:20px;margin:0 0 6px}}p.lead{{color:#aaa;margin:0 0 24px}}         .grid{{display:flex;flex-wrap:wrap;gap:20px}}section{{background:#262523;border:1px solid #3a3835;border-radius:12px;padding:18px;width:260px}}         h2{{font-size:13px;text-transform:uppercase;letter-spacing:.06em;color:#cc785c;margin:0 0 12px}}         .qr svg{{width:220px;height:220px;background:#fff;border-radius:8px;display:block}}         a{{display:block;margin-top:12px;color:#bbb;font:12px ui-monospace,monospace;word-break:break-all}}         .none{{color:#aaa}}code{{color:#eee}}</style></head><body>         <h1>Open this board on your phone</h1>         <p class=lead>Scan a code with a device on the same network. The link carries this machine's access          token - share it only with your own devices.</p><div class=grid>{cards}</div></body></html>"
    );
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

/// One cached `/api/board` response.
#[derive(Clone)]
pub struct BoardCache {
    /// Fingerprint of `board.json` + every ticket file (sizes and mtimes).
    pub signature: u128,
    /// Strong validator handed to the client.
    pub etag: String,
    /// The serialized JSON body.
    pub body: Arc<Vec<u8>>,
}

/// A fingerprint of everything `/api/board` reads: changes whenever the board or
/// any ticket file is added, removed, or rewritten. Only stats files.
fn board_signature(store: &Store) -> u128 {
    use std::time::UNIX_EPOCH;
    let mut sig: u128 = 0;
    let mut add = |m: std::fs::Metadata, salt: u128| {
        let t = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        sig = sig
            .wrapping_mul(1_000_003)
            .wrapping_add(t ^ (m.len() as u128).wrapping_mul(31).wrapping_add(salt));
    };
    if let Ok(m) = std::fs::metadata(store.wipe_dir().join("board.json")) {
        add(m, 1);
    }
    if let Ok(entries) = std::fs::read_dir(store.wipe_dir().join("tickets")) {
        let mut metas: Vec<(String, std::fs::Metadata)> = entries
            .flatten()
            .filter_map(|e| {
                Some((
                    e.file_name().to_string_lossy().into_owned(),
                    e.metadata().ok()?,
                ))
            })
            .collect();
        metas.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, m) in metas {
            let salt = name
                .bytes()
                .fold(7u128, |h, b| h.wrapping_mul(131).wrapping_add(b as u128));
            add(m, salt);
        }
    }
    sig
}

tokio::task_local! {
    /// Per request: may this client's supplied identity be believed? Set by the
    /// auth middleware (local, or the daemon isn't exposed at all).
    pub static TRUSTED: bool;
}

/// Decode a `%XX`/`+`-encoded query value (enough for the `project` path).
pub fn percent_decode(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match (hex(b[i + 1]), hex(b[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi << 4 | lo);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
///
/// When the daemon is **exposed**, the request's `actor` is *not* trusted: a shared
/// bearer token can't tie a request to a specific person, so honoring a
/// client-supplied identity would let a remote caller impersonate anyone. In that
/// mode we fall back to the board's own resolved identity.
fn resolve_actor(state: &AppState, store: &Store, provided: Option<String>) -> String {
    let trusted = TRUSTED.try_with(|t| *t).unwrap_or(!state.exposed);
    let provided = if trusted { provided } else { None };
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
    json!({ "board": board.name, "ids": board.ids, "lists": lists })
}

/// `POST /api/board/translate-ids` - convert a legacy decimal board to hex ticket
/// ids (the UI shows this only for such boards). Runs under the board write lock
/// taken by the mutation middleware.
pub async fn translate_ids(
    State(state): State<AppState>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    let pairs = wipe_core::translate::translate_ids(&store, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "translated": pairs.len() })))
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
        "trash_retention_days": g.trash_retention_days(),
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
    #[serde(default)]
    trash_retention_days: Option<u64>,
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
    if let Some(d) = b.trash_retention_days {
        g.trash_retention_days = Some(d);
    }
    g.save()
        .map_err(|e| ApiError(anyhow::anyhow!("saving config: {e}")))?;
    Ok(Json(json!({
        "ok": true,
        "accent": g.ui_accent,
        "theme": g.ui_theme,
        "default_identity": g.default_identity,
        "prefer_default_identity": g.prefer_default_identity.unwrap_or(false),
        "trash_retention_days": g.trash_retention_days(),
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
///
/// Answers `If-None-Match` with `304 Not Modified` when the board is unchanged,
/// and serves repeat polls from a fingerprint-keyed cache - so an idle UI polling
/// every half second costs a few file stats, not a full board read, transfer,
/// parse and diff.
pub async fn board(
    State(state): State<AppState>,
    Query(q): Query<ProjectQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    let store = store_for(&state, q.project)?;
    let key = store.root().to_path_buf();
    let signature = board_signature(&store);
    let cached = state
        .board_cache
        .lock()
        .ok()
        .and_then(|c| c.get(&key).cloned())
        .filter(|c| c.signature == signature);
    let entry = match cached {
        Some(c) => c,
        None => {
            // Finish an interrupted id translation before showing the board.
            if wipe_core::translate::interrupted(&store) {
                let root = key.clone();
                let _ = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                    let s = Store::open(root)?;
                    let _lock = s.lock()?;
                    wipe_core::translate::translate_ids(&s, Utc::now())?;
                    Ok(())
                })
                .await;
            }
            let (board, view) = ops::board_view(&store)?;
            let body = serde_json::to_vec(&board_json(&board, &view))?;
            let etag = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                body.hash(&mut h);
                format!("\"{:016x}-{:x}\"", h.finish(), body.len())
            };
            let c = BoardCache {
                signature,
                etag,
                body: Arc::new(body),
            };
            if let Ok(mut m) = state.board_cache.lock() {
                m.insert(key, c.clone());
            }
            c
        }
    };
    let not_modified = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == entry.etag);
    let common = [
        (header::ETAG, entry.etag.clone()),
        (header::CACHE_CONTROL, "no-cache".to_string()),
    ];
    if not_modified {
        return Ok((StatusCode::NOT_MODIFIED, common).into_response());
    }
    Ok((
        common,
        [(header::CONTENT_TYPE, "application/json".to_string())],
        (*entry.body).clone(),
    )
        .into_response())
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
    let actor = resolve_actor(&state, &store, b.actor);
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

/// Query for `DELETE /api/tickets/{id}` - `purge=true` deletes permanently
/// (used to discard a never-committed draft), otherwise it soft-deletes to trash.
#[derive(Debug, Deserialize)]
pub struct DeleteTicketQuery {
    project: Option<String>,
    #[serde(default)]
    purge: bool,
}

/// `DELETE /api/tickets/{id}` - soft-delete a ticket: move it to the restorable
/// trash (kept for the user's retention window), removing its card from the board.
/// With `?purge=true` it is deleted outright (no trash). Permanent deletion of an
/// already-trashed ticket happens via `DELETE /api/trash/{id}`.
pub async fn delete_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<DeleteTicketQuery>,
) -> ApiResult {
    let store = store_for(&state, q.project)?;
    if q.purge {
        ops::delete_ticket(&store, &id, Utc::now())?;
        notify(&state);
        return Ok(Json(json!({ "ok": true, "id": id, "trashed": false })));
    }
    let days = wipe_core::GlobalConfig::load().trash_retention_days();
    wipe_core::trash::trash_ticket(&store, &id, days, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id, "trashed": days > 0 })))
}

/// `POST /api/tickets/{id}/duplicate` - copy a ticket onto the same list.
pub async fn duplicate_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    let actor = resolve_actor(&state, &store, None);
    let ticket = ops::duplicate_ticket(&store, &id, &actor, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(ticket)?))
}

/// `GET /api/trash` - list trashed tickets (newest first), with the retention
/// window, after purging anything past it.
pub async fn trash_list(
    State(state): State<AppState>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    let days = wipe_core::GlobalConfig::load().trash_retention_days();
    let entries = wipe_core::trash::list_trash(&store, days, Utc::now())?;
    let items: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "id": e.ticket.id,
                "title": e.ticket.title,
                "list": e.list,
                "labels": e.ticket.labels,
                "deleted_at": e.deleted_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(
        json!({ "retention_days": days, "trash": items, "count": items.len() }),
    ))
}

/// `POST /api/trash/{id}/restore` - restore a trashed ticket onto the board.
pub async fn trash_restore(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    let ticket = wipe_core::trash::restore_ticket(&store, &id, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(ticket)?))
}

/// `DELETE /api/trash/{id}` - permanently delete one trashed ticket.
pub async fn trash_purge(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    let removed = wipe_core::trash::purge_ticket(&store, &id)?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "id": id, "purged": removed })))
}

/// `DELETE /api/trash` - empty the whole trash.
pub async fn trash_empty(
    State(state): State<AppState>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    let n = wipe_core::trash::empty(&store)?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "purged": n })))
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
    let actor = resolve_actor(&state, &store, b.actor);
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
    // Resolve like every other authored action (falls back to the repo VCS/board
    // default) instead of stamping the sentinel "ui", so UI comments are attributed
    // to the real identity and render consistently with CLI/forum authorship.
    let author = resolve_actor(&state, &store, b.author);
    let cid = ops::add_comment(&store, &id, &author, &b.body, Utc::now())?;
    notify(&state);
    Ok(Json(json!({ "ok": true, "ticket": id, "comment": cid })))
}

/// `DELETE /api/tickets/{id}/comments/{comment}` - remove a comment; returns the
/// updated ticket. Project comes from the `?project=` query.
pub async fn delete_comment(
    State(state): State<AppState>,
    Path((id, comment)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    let store = store_for(&state, pq.project)?;
    ops::delete_comment(&store, &id, &comment, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

// --- checklist & acceptance criteria -------------------------------------------
//
// Both tickable surfaces share bodies and inner logic; the route picks the kind.

/// Body for adding a checklist item / acceptance criterion.
#[derive(Debug, Deserialize)]
pub struct ChecklistAddBody {
    project: Option<String>,
    text: String,
}

async fn add_checks_item(
    state: AppState,
    kind: ops::Checks,
    id: String,
    project: Option<String>,
    b: ChecklistAddBody,
) -> ApiResult {
    let store = store_for(&state, project.or(b.project))?;
    ops::checks_add(&store, kind, &id, &b.text, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// `POST /api/tickets/{id}/checklist` - append an item; returns the updated ticket.
pub async fn add_checklist_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistAddBody>,
) -> ApiResult {
    add_checks_item(state, ops::Checks::Checklist, id, pq.project, b).await
}

/// `POST /api/tickets/{id}/acceptance` - append a criterion; returns the ticket.
pub async fn add_acceptance_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistAddBody>,
) -> ApiResult {
    add_checks_item(state, ops::Checks::Acceptance, id, pq.project, b).await
}

/// Body for editing / (un)checking an item. `done` sets the checked state;
/// `text` renames. Either or both may be present.
#[derive(Debug, Deserialize)]
pub struct ChecklistPatchBody {
    project: Option<String>,
    #[serde(default)]
    done: Option<bool>,
    #[serde(default)]
    text: Option<String>,
}

async fn patch_checks_item(
    state: AppState,
    kind: ops::Checks,
    id: String,
    item: String,
    project: Option<String>,
    b: ChecklistPatchBody,
) -> ApiResult {
    let store = store_for(&state, project.or(b.project))?;
    let now = Utc::now();
    if let Some(text) = b.text {
        ops::checks_edit(&store, kind, &id, &item, &text, now)?;
    }
    if let Some(done) = b.done {
        ops::checks_set(&store, kind, &id, &item, Some(done), now)?;
    }
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// `PATCH /api/tickets/{id}/checklist/{item}` - set state and/or text; returns the
/// updated ticket.
pub async fn patch_checklist_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistPatchBody>,
) -> ApiResult {
    patch_checks_item(state, ops::Checks::Checklist, id, item, pq.project, b).await
}

/// `PATCH /api/tickets/{id}/acceptance/{item}` - set state and/or text; returns
/// the updated ticket.
pub async fn patch_acceptance_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistPatchBody>,
) -> ApiResult {
    patch_checks_item(state, ops::Checks::Acceptance, id, item, pq.project, b).await
}

async fn delete_checks_item(
    state: AppState,
    kind: ops::Checks,
    id: String,
    item: String,
    project: Option<String>,
) -> ApiResult {
    let store = store_for(&state, project)?;
    ops::checks_remove(&store, kind, &id, &item, Utc::now())?;
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
    delete_checks_item(state, ops::Checks::Checklist, id, item, pq.project).await
}

/// `DELETE /api/tickets/{id}/acceptance/{item}` - remove a criterion; returns the
/// ticket. The project is taken from the `?project=` query.
pub async fn delete_acceptance_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult {
    delete_checks_item(state, ops::Checks::Acceptance, id, item, pq.project).await
}

/// Body for reordering an item.
#[derive(Debug, Deserialize)]
pub struct ChecklistMoveBody {
    project: Option<String>,
    index: usize,
}

async fn move_checks_item(
    state: AppState,
    kind: ops::Checks,
    id: String,
    item: String,
    project: Option<String>,
    b: ChecklistMoveBody,
) -> ApiResult {
    let store = store_for(&state, project.or(b.project))?;
    ops::checks_move(&store, kind, &id, &item, b.index, Utc::now())?;
    notify(&state);
    Ok(Json(serde_json::to_value(store.load_ticket(&id)?)?))
}

/// `POST /api/tickets/{id}/checklist/{item}/move` - reorder; returns the ticket.
pub async fn move_checklist_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistMoveBody>,
) -> ApiResult {
    move_checks_item(state, ops::Checks::Checklist, id, item, pq.project, b).await
}

/// `POST /api/tickets/{id}/acceptance/{item}/move` - reorder; returns the ticket.
pub async fn move_acceptance_item(
    State(state): State<AppState>,
    Path((id, item)): Path<(String, String)>,
    Query(pq): Query<ProjectQuery>,
    Json(b): Json<ChecklistMoveBody>,
) -> ApiResult {
    move_checks_item(state, ops::Checks::Acceptance, id, item, pq.project, b).await
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
    let actor = resolve_actor(&state, &store, b.actor);
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
    let actor = resolve_actor(&state, &store, None);
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

/// Body for attaching a file that already exists on the local filesystem.
#[derive(Debug, Deserialize)]
pub struct AttachPathBody {
    project: Option<String>,
    path: String,
}

/// Best-effort MIME from a file name's extension (metadata only; storage is
/// content-hashed regardless).
fn guess_mime(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// Refuse unless the request comes from this machine (or the daemon isn't
/// exposed). Reading a *path* makes the daemon open a file on its own disk; a
/// remote client - even one holding the token - must not be able to point it
/// at arbitrary host files.
fn require_local(state: &AppState) -> Result<(), ApiError> {
    if TRUSTED.try_with(|t| *t).unwrap_or(!state.exposed) {
        Ok(())
    } else {
        Err(ApiError(anyhow::anyhow!(
            "local file paths can only be used from the machine running `wipe serve` - upload the file instead"
        )))
    }
}

/// Query for `GET /api/local-file`.
#[derive(Debug, Deserialize)]
pub struct LocalFileQuery {
    path: String,
}

/// `GET /api/local-file?path=` - stream a local file for an in-place preview of a
/// pasted path before it is attached. This-machine only; previewable media types
/// (image/video/audio/pdf/text) only; capped at 50 MB.
pub async fn local_file(
    State(state): State<AppState>,
    Query(q): Query<LocalFileQuery>,
) -> Result<Response, ApiError> {
    require_local(&state)?;
    let path = std::path::PathBuf::from(q.path.trim());
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let mime = guess_mime(&name);
    let previewable = [
        "image/",
        "video/",
        "audio/",
        "text/",
        "application/pdf",
        "application/json",
    ]
    .iter()
    .any(|p| mime.starts_with(p));
    if !previewable || !path.is_file() {
        return Err(ApiError(anyhow::anyhow!(
            "no preview for {}",
            path.display()
        )));
    }
    if std::fs::metadata(&path)
        .map(|m| m.len())
        .unwrap_or(u64::MAX)
        > 50 * 1024 * 1024
    {
        return Err(ApiError(anyhow::anyhow!("too large to preview")));
    }
    let bytes = std::fs::read(&path)?;
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}

/// `POST /api/tickets/{id}/attachments/path` - attach a file that already lives on
/// the local filesystem (e.g. a path pasted into the editor). The daemon (which
/// runs locally) reads it and applies the same content-hash dedupe + in-repo
/// referencing as any attach, so pasting the same path twice never double-attaches.
pub async fn attach_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<AttachPathBody>,
) -> ApiResult {
    require_local(&state)?;
    let store = store_for(&state, b.project)?;
    let path = std::path::PathBuf::from(b.path.trim());
    if !path.is_file() {
        return Err(ApiError(anyhow::anyhow!(
            "not a file on this machine: {}",
            path.display()
        )));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let bytes = std::fs::read(&path)
        .map_err(|e| ApiError(anyhow::anyhow!("cannot read {}: {e}", path.display())))?;
    let max = store.load_settings()?.max_attachment_mb * 1024 * 1024;
    if bytes.len() as u64 > max {
        return Err(ApiError(anyhow::anyhow!(
            "attachment is {:.1} MB, over the {} MB limit",
            bytes.len() as f64 / 1_048_576.0,
            max / 1024 / 1024
        )));
    }
    let actor = resolve_actor(&state, &store, None);
    let att = ops::add_attachment(
        &store,
        &id,
        &name,
        &bytes,
        guess_mime(&name),
        &actor,
        Utc::now(),
    )?;
    notify(&state);
    Ok(Json(serde_json::to_value(att)?))
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
    let actor = resolve_actor(&state, &store, b.actor);
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
    let mut threads: Vec<Value> = all
        .iter()
        .filter(|p| p.depth == 0)
        .map(|r| {
            let thread_posts: Vec<&_> = all.iter().filter(|p| p.thread_id == r.thread_id).collect();
            let posts = thread_posts.len();
            // Last activity = the most recent post anywhere in the thread; its author
            // is who bumped it. Drives the sidebar's activity sort + "· 2m ago".
            let last = thread_posts
                .iter()
                .max_by_key(|p| p.created)
                .copied()
                .unwrap_or(r);
            // A one-line preview from the root post so the sidebar rows are legible
            // at a glance (collapse whitespace, trim).
            let snippet: String = r
                .body
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(140)
                .collect();
            // Distinct authors anywhere in the thread, so the UI can filter by "who
            // engaged" (participated), not just the OP.
            let mut participants: Vec<String> =
                thread_posts.iter().map(|p| p.author.clone()).collect();
            participants.sort();
            participants.dedup();
            json!({
                "id": r.thread_id,
                "title": r.thread_title,
                "author": r.author,
                "labels": r.labels,
                "posts": posts,
                "created": r.created,
                "updated": last.created,
                "last_author": last.author,
                "snippet": snippet,
                "participants": participants,
            })
        })
        .collect();
    // Newest activity first (the daemon sorts so every client gets it consistently).
    threads.sort_by(|a, b| b["updated"].as_str().cmp(&a["updated"].as_str()));
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
    let actor = resolve_actor(&state, &store, b.actor);
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
    let actor = resolve_actor(&state, &store, b.actor);
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
