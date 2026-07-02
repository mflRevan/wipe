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

use axum::routing::{get, post};
use axum::Router;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use wipe_core::model::Exposure;

pub use api::AppState;
pub use registry::{list as list_projects, ProjectEntry};

/// Configuration for a `wipe serve` invocation.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Project root to serve (the directory containing `.wipe`).
    pub root: PathBuf,
    /// TCP port to bind.
    pub port: u16,
    /// How the daemon is exposed beyond localhost.
    pub expose: Exposure,
    /// Whether to open a browser once bound (best-effort; currently a hint).
    pub open: bool,
}

/// Build the application router for a given state.
fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(api::health))
        .route("/api/projects", get(api::projects))
        .route("/api/board", get(api::board))
        .route("/api/history", get(api::history))
        .route("/api/board/at", get(api::board_at))
        .route("/api/tickets", post(api::create_ticket))
        .route("/api/tickets/{id}/move", post(api::move_ticket))
        .route("/api/tickets/{id}/comments", post(api::add_comment))
        .route("/ws", get(api::ws_handler))
        .fallback(assets::static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Start the daemon and serve until the process is stopped (Ctrl-C).
pub async fn serve(cfg: ServeConfig) -> anyhow::Result<()> {
    registry::register(&cfg.root);

    let (tx, _rx) = broadcast::channel::<String>(64);
    let state = AppState {
        current: cfg.root.clone(),
        tx: tx.clone(),
    };

    // Watch `.wipe` for live updates; keep the watcher alive for the whole serve.
    let _watcher = watch::spawn(&cfg.root.join(".wipe"), tx.clone());
    if _watcher.is_err() {
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
    println!("wipe UI serving on http://{shown}  (Ctrl-C to stop)");
    if cfg.open {
        open_browser(&format!("http://{shown}"));
    }

    let app = router(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Best-effort: open `url` in the user's default browser.
fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
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
        AppState { current: root, tx }
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
}
