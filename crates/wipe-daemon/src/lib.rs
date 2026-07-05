//! The wipe local daemon: an `axum` server that exposes the board over HTTP/WS
//! and serves the embedded human UI. Started by `wipe serve`.
//!
//! Collaboration remains git-only; this daemon is a *local* convenience for the
//! human UX. It records each served project in a machine-wide registry so the UI
//! can list every board you have opened.

mod api;
mod assets;
mod registry;
mod watch;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use axum::routing::{get, patch, post, put};
use axum::Router;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use wipe_core::model::Exposure;

pub use api::AppState;
pub use registry::{list as list_projects, ProjectEntry};

/// Configuration for a `wipe serve` invocation.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Project root to open by default (the directory containing `.wipe`). `None`
    /// when serving purely as a global viewer from outside any board - the UI then
    /// lists every registered project and the user picks one.
    pub root: Option<PathBuf>,
    /// TCP port to bind.
    pub port: u16,
    /// How the daemon is exposed beyond localhost.
    pub expose: Exposure,
    /// Whether to open a browser once bound (best-effort; currently a hint).
    pub open: bool,
    /// If set, the daemon shuts itself down after this long with no connected UI
    /// clients - so auto-served daemons leave no overhead once the tab is closed.
    pub idle_timeout: Option<std::time::Duration>,
}

/// Build the application router for a given state.
fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(api::health))
        .route("/api/config", get(api::app_config).patch(api::patch_config))
        .route("/api/scan", post(api::rescan))
        .route("/api/projects", get(api::projects))
        .route("/api/board", get(api::board))
        .route("/api/history", get(api::history))
        .route("/api/board/at", get(api::board_at))
        .route("/api/definitions", get(api::definitions))
        .route("/api/graph", get(api::graph))
        .route("/api/labels", post(api::create_label))
        .route(
            "/api/labels/{name}",
            patch(api::recolor_label).delete(api::delete_label),
        )
        .route("/api/lists", post(api::add_list))
        .route(
            "/api/lists/{id}",
            patch(api::rename_list).delete(api::remove_list),
        )
        .route("/api/lists/{id}/move", post(api::move_list))
        .route("/api/identities", get(api::identities))
        .route(
            "/api/identities/{id}",
            put(api::put_identity).delete(api::delete_identity),
        )
        .route("/api/tickets", post(api::create_ticket))
        .route("/api/tickets/{id}", patch(api::patch_ticket))
        .route("/api/tickets/{id}/move", post(api::move_ticket))
        .route("/api/tickets/{id}/comments", post(api::add_comment))
        .route(
            "/api/tickets/{id}/attachments",
            post(api::upload_attachment).delete(api::delete_attachment),
        )
        .route("/api/media/{*path}", get(api::serve_media))
        .route("/api/forum", get(api::forum_list).post(api::forum_create))
        .route("/api/forum/search", get(api::forum_search))
        .route(
            "/api/forum/{id}",
            get(api::forum_thread)
                .patch(api::forum_edit)
                .delete(api::forum_delete),
        )
        .route("/api/forum/{id}/reply", post(api::forum_reply))
        .route("/ws", get(api::ws_handler))
        .fallback(assets::static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Start the daemon and serve until the process is stopped (Ctrl-C).
pub async fn serve(cfg: ServeConfig) -> anyhow::Result<()> {
    if let Some(root) = &cfg.root {
        registry::register(root);
    }

    let (tx, _rx) = broadcast::channel::<String>(64);
    let clients = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = AppState {
        current: cfg.root.clone(),
        tx: tx.clone(),
        clients: clients.clone(),
    };

    // Watch the launch project's `.wipe` for live updates; keep the watcher alive
    // for the whole serve. (Global-viewer mode has no single dir to watch; the UI
    // still refetches on demand.)
    let _watcher = cfg
        .root
        .as_ref()
        .map(|root| watch::spawn(&root.join(".wipe"), tx.clone()));
    if matches!(_watcher, Some(Err(_))) {
        eprintln!("warning: file watching unavailable; live updates disabled");
    }

    let ip = match cfg.expose {
        Exposure::None => Ipv4Addr::LOCALHOST,
        Exposure::Tailscale | Exposure::Proxy => Ipv4Addr::UNSPECIFIED,
    };
    let addr = SocketAddr::from((ip, cfg.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;

    let shown = if bound.ip().is_unspecified() {
        SocketAddr::from((Ipv4Addr::LOCALHOST, bound.port()))
    } else {
        bound
    };
    match cfg.idle_timeout {
        Some(d) => println!(
            "wipe UI serving on http://{shown}  (Ctrl-C to stop; auto-stops after {}s idle)",
            d.as_secs()
        ),
        None => println!("wipe UI serving on http://{shown}  (Ctrl-C to stop)"),
    }
    if cfg.open {
        open_browser(&format!("http://{shown}"));
    }

    let app = router(state);
    let idle = cfg.idle_timeout;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown_signal(clients, idle).await })
        .await?;
    Ok(())
}

/// Resolve when the daemon should stop: on Ctrl-C, or - if an idle timeout is
/// configured - once there have been no connected UI clients for that long.
async fn shutdown_signal(
    clients: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    idle: Option<Duration>,
) {
    tokio::select! {
        _ = async { let _ = tokio::signal::ctrl_c().await; } => {}
        _ = idle_watcher(clients, idle) => {
            println!("wipe: idle with no viewers; shutting down.");
        }
    }
}

/// Completes once the daemon has been idle (zero clients) for `timeout`. If no
/// timeout is set, never completes.
async fn idle_watcher(
    clients: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    timeout: Option<Duration>,
) {
    use std::sync::atomic::Ordering;
    let Some(timeout) = timeout else {
        std::future::pending::<()>().await;
        return;
    };
    let mut idle_since = Some(std::time::Instant::now());
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    loop {
        tick.tick().await;
        if clients.load(Ordering::SeqCst) > 0 {
            idle_since = None;
        } else {
            let since = idle_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= timeout {
                return;
            }
        }
    }
}

/// Best-effort: open `url` in the user's default browser.
fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    fn test_state(root: PathBuf) -> AppState {
        let (tx, _rx) = broadcast::channel(8);
        AppState {
            current: Some(root),
            tx,
            clients: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    #[tokio::test]
    async fn health_and_board_endpoints() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Daemon Test", chrono::Utc::now()).unwrap();
        wipe_core::ops::create_ticket(
            &store,
            wipe_core::ops::NewTicket {
                title: "Hello".into(),
                ..Default::default()
            },
            "tester",
            chrono::Utc::now(),
        )
        .unwrap();

        let app = router(test_state(store.root().to_path_buf()));

        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let board = app
            .oneshot(
                Request::builder()
                    .uri("/api/board")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(board.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(board.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["board"], "Daemon Test");
        assert_eq!(v["lists"][0]["tickets"][0]["title"], "Hello");
    }

    use wipe_core::Store;

    /// Percent-encode a string for use as a query-parameter value.
    fn enc(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    }

    async fn board_titles(app: &Router, project: &str) -> Vec<String> {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/board?project={}", enc(project)))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["lists"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|l| l["tickets"].as_array().unwrap().clone())
            .map(|t| t["title"].as_str().unwrap().to_string())
            .collect()
    }

    /// A mutation naming a project via `?project=` must hit THAT board, never the
    /// daemon's launch project. Guards the silent-write-to-served-board bug.
    #[tokio::test]
    async fn mutations_target_the_requested_project_not_the_served_one() {
        let served = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let served_store = Store::init(served.path(), "Served", chrono::Utc::now()).unwrap();
        let other_store = Store::init(other.path(), "Other", chrono::Utc::now()).unwrap();
        let other_root = other_store.root().display().to_string();

        // Daemon launched in the "Served" board.
        let app = router(test_state(served_store.root().to_path_buf()));

        // Create a ticket while viewing the OTHER board (project passed in query).
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/tickets?project={}", enc(&other_root)))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"title":"lands in other"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // It must appear in "Other" and leave "Served" empty.
        assert_eq!(
            board_titles(&app, &other_root).await,
            vec!["lands in other".to_string()]
        );
        assert!(
            board_titles(&app, &served_store.root().display().to_string())
                .await
                .is_empty()
        );
    }
}
