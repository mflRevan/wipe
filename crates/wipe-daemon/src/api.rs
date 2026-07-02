//! HTTP + WebSocket API handlers over `wipe-core`.

use std::path::PathBuf;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use wipe_core::model::{Board, Ticket};
use wipe_core::ops::{self, NewTicket};
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
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    list: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    assignees: Vec<String>,
}

/// `POST /api/tickets`
pub async fn create_ticket(
    State(state): State<AppState>,
    Json(b): Json<CreateTicketBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
    let spec = NewTicket {
        title: b.title,
        body: b.body,
        kind: b.kind,
        priority: b.priority,
        list: b.list,
        labels: b.labels,
        tags: b.tags,
        assignees: b.assignees,
    };
    let ticket = ops::create_ticket(&store, spec, Utc::now())?;
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
}

/// `POST /api/tickets/{id}/move`
pub async fn move_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<MoveBody>,
) -> ApiResult {
    let store = store_for(&state, b.project)?;
    ops::move_ticket(&store, &id, &b.to, b.pos, Utc::now())?;
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
