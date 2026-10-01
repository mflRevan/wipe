//! The wipe local daemon: an `axum` server that exposes the board over HTTP/WS
//! and serves the embedded human UI. Started by `wipe serve`.
//!
//! Collaboration remains git-only; this daemon is a *local* convenience for the
//! human UX. It records each served project in a machine-wide registry so the UI
//! can list every board you have opened.

mod api;
mod assets;
mod net;
mod registry;
mod watch;

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde_json::json;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use wipe_core::model::Exposure;

pub use api::AppState;
pub use registry::{list as list_projects, ProjectEntry};

/// Configuration for a `wipe serve` invocation.
#[derive(Clone)]
pub struct ServeConfig {
    /// Project root to open by default (the directory containing `.wipe`). `None`
    /// when serving purely as a global viewer from outside any board - the UI then
    /// lists every registered project and the user picks one.
    pub root: Option<PathBuf>,
    /// TCP port to bind.
    pub port: u16,
    /// How the daemon is exposed beyond localhost.
    pub expose: Exposure,
    /// Bind exactly this address instead of the mode's default plan.
    pub host: Option<IpAddr>,
    /// Print a QR code of the first remote URL (for opening it on a phone).
    pub qr: bool,
    /// Whether to open a browser once bound (best-effort; currently a hint).
    pub open: bool,
    /// If set, the daemon shuts itself down after this long with no connected UI
    /// clients - so auto-served daemons leave no overhead once the tab is closed.
    pub idle_timeout: Option<std::time::Duration>,
    /// Stop serving when this flips to `true` (used by the tray app's Quit).
    pub stop: Option<tokio::sync::watch::Receiver<bool>>,
    /// Called once the sockets are bound, with the URLs being served (the tray
    /// uses it for its menu). The first entry is this machine's URL.
    pub on_ready: Option<std::sync::Arc<dyn Fn(Vec<ShownUrl>) + Send + Sync>>,
}

pub use net::ShownUrl;

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
        .route("/api/board/translate-ids", post(api::translate_ids))
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
        .route(
            "/api/tickets/{id}",
            patch(api::patch_ticket).delete(api::delete_ticket),
        )
        .route("/api/tickets/{id}/move", post(api::move_ticket))
        .route("/api/tickets/{id}/duplicate", post(api::duplicate_ticket))
        .route("/api/trash", get(api::trash_list).delete(api::trash_empty))
        .route("/api/trash/{id}", delete(api::trash_purge))
        .route("/api/trash/{id}/restore", post(api::trash_restore))
        .route("/api/tickets/{id}/comments", post(api::add_comment))
        .route(
            "/api/tickets/{id}/comments/{comment}",
            delete(api::delete_comment),
        )
        .route("/api/tickets/{id}/checklist", post(api::add_checklist_item))
        .route(
            "/api/tickets/{id}/checklist/{item}",
            patch(api::patch_checklist_item).delete(api::delete_checklist_item),
        )
        .route(
            "/api/tickets/{id}/checklist/{item}/move",
            post(api::move_checklist_item),
        )
        .route(
            "/api/tickets/{id}/acceptance",
            post(api::add_acceptance_item),
        )
        .route(
            "/api/tickets/{id}/acceptance/{item}",
            patch(api::patch_acceptance_item).delete(api::delete_acceptance_item),
        )
        .route(
            "/api/tickets/{id}/acceptance/{item}/move",
            post(api::move_acceptance_item),
        )
        .route(
            "/api/tickets/{id}/attachments",
            post(api::upload_attachment).delete(api::delete_attachment),
        )
        .route("/api/tickets/{id}/attachments/path", post(api::attach_path))
        .route("/api/media/{*path}", get(api::serve_media))
        .route("/api/local-file", get(api::local_file))
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
        .route("/connect", get(api::connect_page))
        .fallback(assets::static_handler)
        // Exposed daemons only answer cross-origin calls from this machine's own
        // front-ends (the desktop app, a local dev server) and gate the API/WS on a
        // bearer token for remote peers; localhost-only keeps the permissive policy.
        .layer(if state.exposed {
            CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::predicate(|o, _| {
                    local_origin(o.to_str().unwrap_or(""))
                }))
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
                .expose_headers([axum::http::header::ETAG])
        } else {
            CorsLayer::permissive()
        })
        .layer(middleware::from_fn_with_state(state.clone(), write_lock))
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

/// Whether a browser `Origin` belongs to a front-end on this machine: a loopback
/// http(s) origin (any port) or the Tauri desktop shell.
fn local_origin(origin: &str) -> bool {
    if matches!(
        origin,
        "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
    ) {
        return true;
    }
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = match rest.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => rest.split(':').next().unwrap_or(""),
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// Whether the request came from this machine (loopback peer address). Requests
/// without connection info (in-process tests) count as remote.
fn from_loopback(req: &Request) -> bool {
    let ext = req.extensions();
    ext.get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(a)| *a)
        // What `MockConnectInfo` (tests) provides instead.
        .or_else(|| {
            ext.get::<axum::extract::connect_info::MockConnectInfo<SocketAddr>>()
                .map(|m| m.0)
        })
        .is_some_and(|a| a.ip().to_canonical().is_loopback())
}

/// Auth gate: when a token is configured (exposed mode), every `/api` request
/// (except the unauthenticated health probe) and the `/ws` upgrade must carry the
/// token as `Authorization: Bearer <t>` or a `?token=<t>` query parameter -
/// unless it comes from this machine and loopback is trusted (every mode but
/// `proxy`, where all traffic arrives via loopback). Also records, for the
/// request, whether its client-supplied identity may be believed.
async fn require_token(State(state): State<api::AppState>, req: Request, next: Next) -> Response {
    let local = state.trust_loopback && from_loopback(&req);
    if let Some(token) = state.token.clone() {
        let path = req.uri().path();
        let guarded = (path.starts_with("/api") && path != "/api/health") || path == "/ws";
        if guarded && !local && !request_has_token(&req, &token) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "ok": false, "error": "missing or invalid token" })),
            )
                .into_response();
        }
    }
    let trusted = !state.exposed || local;
    api::TRUSTED.scope(trusted, next.run(req)).await
}

/// Serialize board writes: every mutating API request holds the target board's
/// write lock (shared with the CLI) for its duration, so a UI edit and an agent's
/// `wipe` command never interleave a read-modify-write.
async fn write_lock(State(state): State<api::AppState>, req: Request, next: Next) -> Response {
    let mutating = !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    if !mutating || !req.uri().path().starts_with("/api/") {
        return next.run(req).await;
    }
    let project = req.uri().query().and_then(|q| {
        q.split('&')
            .find_map(|kv| kv.strip_prefix("project="))
            .map(api::percent_decode)
    });
    let root = project.map(PathBuf::from).or_else(|| state.current.clone());
    let guard = match root.and_then(|r| wipe_core::Store::open(r).ok()) {
        Some(store) => tokio::task::spawn_blocking(move || store.lock().ok())
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let res = next.run(req).await;
    drop(guard);
    res
}

/// Whether `req` presents the expected bearer `token`, via the `Authorization`
/// header or a `token=` query parameter (the latter lets a shared URL carry it).
fn request_has_token(req: &Request, token: &str) -> bool {
    if let Some(auth) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(bearer) = auth
            .strip_prefix("Bearer ")
            .or_else(|| auth.strip_prefix("bearer "))
        {
            if bearer.trim() == token {
                return true;
            }
        }
    }
    if let Some(q) = req.uri().query() {
        for pair in q.split('&') {
            if let Some(v) = pair.strip_prefix("token=") {
                if v == token {
                    return true;
                }
            }
        }
    }
    false
}

/// Resolve the bearer token for an exposed serve: `$WIPE_TOKEN` wins; then a
/// token already saved in the board's `daemon.token` (pre-0.4 boards); otherwise
/// this machine's own token, generated once and kept in the user config dir so
/// the phone bookmark stays valid across restarts. It is deliberately never
/// written into the git-tracked board: a secret must not ride along in commits.
fn resolve_token(root: Option<&PathBuf>) -> String {
    if let Ok(t) = std::env::var("WIPE_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return t;
        }
    }
    if let Some(t) = root
        .and_then(|r| wipe_core::Store::open(r).ok())
        .and_then(|s| s.load_settings().ok())
        .and_then(|st| st.daemon.token)
        .filter(|t| !t.trim().is_empty())
    {
        return t;
    }
    let path =
        wipe_core::GlobalConfig::path().and_then(|p| p.parent().map(|d| d.join("serve-token")));
    if let Some(t) = path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        return t;
    }
    let t = wipe_core::id::token();
    if let Some(p) = &path {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(p, &t).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
    t
}

/// Start the daemon and serve until the process is stopped (Ctrl-C).
pub async fn serve(cfg: ServeConfig) -> anyhow::Result<()> {
    if let Some(root) = &cfg.root {
        registry::register(root);
    }

    let addrs = net::bind_plan(cfg.expose, cfg.host, cfg.port)?;
    let (tx, _rx) = broadcast::channel::<String>(64);
    let clients = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let proxy = matches!(cfg.expose, Exposure::Proxy);
    let exposed = net::is_remote(&addrs) || proxy;
    // Reachable-from-elsewhere daemons require a token from remote clients; a
    // loopback-only serve stays auth-free.
    let token = if exposed {
        Some(resolve_token(cfg.root.as_ref()))
    } else {
        None
    };
    let connect_urls: std::sync::Arc<std::sync::OnceLock<Vec<ShownUrl>>> = Default::default();
    let state = AppState {
        current: cfg.root.clone(),
        tx: tx.clone(),
        clients: clients.clone(),
        token: token.clone(),
        exposed,
        trust_loopback: !proxy,
        board_cache: Default::default(),
        connect_urls: connect_urls.clone(),
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

    let mut listeners = Vec::new();
    let mut bound = Vec::new();
    for addr in &addrs {
        let l = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            anyhow::anyhow!(
                "cannot listen on {addr}: {e} (another server on this port? try --port)"
            )
        })?;
        bound.push(l.local_addr()?);
        listeners.push(l);
    }
    // `localhost` resolves to ::1 first on Windows and most Linux setups; with only
    // an IPv4 socket every request from the UI waited ~200 ms for that attempt to
    // fail before falling back. Listen on the IPv6 twin too (best-effort: skipped
    // where IPv6 is off, or where the IPv4 socket is already dual-stack).
    for addr in net::ipv6_twins(&bound) {
        if let Ok(l) = tokio::net::TcpListener::bind(addr).await {
            listeners.push(l);
        }
    }

    // Remote URLs carry the token so the first open authenticates; the UI keeps it
    // and sends it on every later API/WS call.
    let urls = net::urls(&bound, token.as_deref(), proxy);
    let _ = connect_urls.set(urls.clone());
    if let Some(cb) = &cfg.on_ready {
        cb(urls.clone());
    }
    match cfg.idle_timeout {
        Some(d) => println!(
            "wipe UI serving  (Ctrl-C to stop; auto-stops after {}s idle)",
            d.as_secs()
        ),
        None => println!("wipe UI serving  (Ctrl-C to stop)"),
    }
    let width = urls.iter().map(|u| u.label.len()).max().unwrap_or(0);
    for u in &urls {
        println!("  {:<width$}  {}", u.label, u.url);
    }
    if cfg.expose == Exposure::Tailscale {
        if let (Some(name), Some(ts)) = (net::tailscale_dns_name(), token.as_deref()) {
            println!(
                "  {:<width$}  http://{name}:{}/?token={ts}",
                "tailscale dns",
                bound[0].port()
            );
        }
    }
    let remote: Vec<&net::ShownUrl> = urls.iter().filter(|u| u.label != "this machine").collect();
    if exposed {
        if cfg.qr {
            if let Some(q) = remote.first().and_then(|u| net::qr(&u.url)) {
                println!("\n  scan to open on your phone ({}):", remote[0].label);
                for line in q.lines() {
                    println!("  {line}");
                }
            }
        }
        println!(
            "  anyone holding a token URL can read and edit this board - share it only with \
             trusted devices. this machine only: `wipe serve --local`."
        );
        println!(
            "  QR codes + links for your phone: http://localhost:{}/connect (open it on this machine)",
            bound[0].port()
        );
    }
    if cfg.open {
        open_browser(&urls[0].url);
    }

    let app = router(state);
    let idle = cfg.idle_timeout;
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let mut servers = Vec::new();
    for l in listeners {
        let mut rx = stop_rx.clone();
        let svc = app
            .clone()
            .into_make_service_with_connect_info::<SocketAddr>();
        servers.push(tokio::spawn(async move {
            axum::serve(l, svc)
                .with_graceful_shutdown(async move {
                    let _ = rx.wait_for(|stop| *stop).await;
                })
                .await
        }));
    }
    shutdown_signal(clients, idle, cfg.stop.clone()).await;
    let _ = stop_tx.send(true);
    for s in servers {
        s.await??;
    }
    Ok(())
}

/// Resolve when the daemon should stop: on Ctrl-C, or - if an idle timeout is
/// configured - once there have been no connected UI clients for that long.
async fn shutdown_signal(
    clients: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    idle: Option<Duration>,
    stop: Option<tokio::sync::watch::Receiver<bool>>,
) {
    tokio::select! {
        _ = async { let _ = tokio::signal::ctrl_c().await; } => {}
        _ = async {
            match stop {
                Some(mut rx) => { let _ = rx.wait_for(|s| *s).await; }
                None => std::future::pending::<()>().await,
            }
        } => {}
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
            token: None,
            exposed: false,
            trust_loopback: true,
            board_cache: Default::default(),
            connect_urls: Default::default(),
        }
    }

    /// An exposed state guarded by `token`, for auth tests.
    fn test_state_exposed(root: PathBuf, token: &str) -> AppState {
        let (tx, _rx) = broadcast::channel(8);
        AppState {
            current: Some(root),
            tx,
            clients: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            token: Some(token.to_string()),
            exposed: true,
            trust_loopback: true,
            board_cache: Default::default(),
            connect_urls: Default::default(),
        }
    }

    #[tokio::test]
    async fn exposed_daemon_requires_token() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Auth", chrono::Utc::now()).unwrap();
        let app = router(test_state_exposed(store.root().to_path_buf(), "secret123"));

        // Health stays open (liveness probes need no token).
        let h = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(h.status(), StatusCode::OK);

        // A guarded endpoint without a token is rejected.
        let no = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/board")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(no.status(), StatusCode::UNAUTHORIZED);

        // Bearer header authorizes.
        let hdr = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/board")
                    .header("authorization", "Bearer secret123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(hdr.status(), StatusCode::OK);

        // `?token=` query authorizes too (how the shared URL carries it).
        let q = app
            .oneshot(
                Request::builder()
                    .uri("/api/board?token=secret123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(q.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn board_polls_get_304_until_the_board_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Etag", chrono::Utc::now()).unwrap();
        let app = router(test_state(store.root().to_path_buf()));
        let get = |etag: Option<String>| {
            let app = app.clone();
            async move {
                let mut r = Request::builder().uri("/api/board");
                if let Some(e) = etag {
                    r = r.header("if-none-match", e);
                }
                app.oneshot(r.body(Body::empty()).unwrap()).await.unwrap()
            }
        };
        let first = get(None).await;
        assert_eq!(first.status(), StatusCode::OK);
        let etag = first.headers()["etag"].to_str().unwrap().to_string();
        assert_eq!(
            get(Some(etag.clone())).await.status(),
            StatusCode::NOT_MODIFIED
        );

        wipe_core::ops::create_ticket(
            &store,
            wipe_core::ops::NewTicket {
                title: "new".into(),
                ..Default::default()
            },
            "t",
            chrono::Utc::now(),
        )
        .unwrap();
        let after = get(Some(etag.clone())).await;
        assert_eq!(
            after.status(),
            StatusCode::OK,
            "a change must invalidate the etag"
        );
        assert_ne!(after.headers()["etag"].to_str().unwrap(), etag);
        let bytes = axum::body::to_bytes(after.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["lists"][0]["tickets"][0]["id"], "T-001");
        assert_eq!(v["ids"], "hex");
    }

    #[test]
    fn only_local_front_ends_are_cors_origins() {
        for ok in [
            "http://localhost:5173",
            "http://127.0.0.1:6737",
            "http://[::1]:8080",
            "tauri://localhost",
            "http://tauri.localhost",
        ] {
            assert!(local_origin(ok), "{ok}");
        }
        for bad in [
            "http://evil.example",
            "http://localhost.evil.example",
            "http://192.168.1.5:6737",
            "",
        ] {
            assert!(!local_origin(bad), "{bad}");
        }
    }

    /// Requests from this machine skip the token (unless behind a proxy);
    /// remote ones still need it.
    #[tokio::test]
    async fn loopback_is_trusted_except_behind_a_proxy() {
        use axum::extract::connect_info::MockConnectInfo;
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Auth", chrono::Utc::now()).unwrap();
        let get = |app: Router| async move {
            app.oneshot(
                Request::builder()
                    .uri("/api/board")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        };
        let state = test_state_exposed(store.root().to_path_buf(), "secret123");
        let local =
            router(state.clone()).layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 5555))));
        assert_eq!(get(local).await, StatusCode::OK);

        let remote = router(state.clone())
            .layer(MockConnectInfo(SocketAddr::from(([192, 168, 1, 9], 5555))));
        assert_eq!(get(remote).await, StatusCode::UNAUTHORIZED);

        let mut proxied = state;
        proxied.trust_loopback = false;
        let behind_proxy =
            router(proxied).layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 5555))));
        assert_eq!(get(behind_proxy).await, StatusCode::UNAUTHORIZED);
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
