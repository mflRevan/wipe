//! Serve the embedded board UI.
//!
//! The UI is baked in from the `assets/` directory at compile time. That
//! directory is a *build-staging* location populated by `scripts/embed-ui`
//! (which builds `apps/desktop` and copies its output here); it is gitignored.
//! When it hasn't been populated — e.g. a plain `cargo build` with no UI — we
//! fall back to a compiled-in placeholder page so the daemon always works.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
struct Assets;

/// Shown when no built UI has been embedded yet.
const FALLBACK_HTML: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>wipe</title><style>
:root{color-scheme:dark}
body{margin:0;min-height:100vh;display:grid;place-items:center;
font:15px/1.6 ui-sans-serif,system-ui,sans-serif;background:#0b0b0f;color:#e7e7ea}
.card{max-width:34rem;padding:2rem;text-align:center}
h1{font-size:2rem;margin:0 0 .5rem;letter-spacing:-.02em}
.accent{color:#8b5cf6}code{background:#17171d;padding:.15rem .4rem;border-radius:.35rem}
p{color:#a1a1aa}</style></head>
<body><div class="card"><h1><span class="accent">wipe</span> is running</h1>
<p>The daemon is live and its API is under <code>/api</code>. The board UI is
bundled from the desktop app at build time; this placeholder ships until that
build is embedded (run <code>scripts/embed-ui</code>).</p>
<p>Try <code>/api/health</code> or <code>/api/board</code>.</p></div></body></html>"#;

/// Axum fallback handler: serve embedded static files with a SPA fallback.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            [(header::CONTENT_TYPE, mime.as_ref())],
            Body::from(content.data.into_owned()),
        )
            .into_response();
    }

    // SPA fallback: serve the built index.html for unknown routes, or the
    // compiled-in placeholder when no UI has been embedded.
    match Assets::get("index.html") {
        Some(content) => (
            [(header::CONTENT_TYPE, "text/html")],
            Body::from(content.data.into_owned()),
        )
            .into_response(),
        None => {
            if uri.path().starts_with("/api") || uri.path() == "/ws" {
                return (StatusCode::NOT_FOUND, "not found").into_response();
            }
            ([(header::CONTENT_TYPE, "text/html")], FALLBACK_HTML).into_response()
        }
    }
}
