//! Filesystem watching: turn `.wipe` changes into broadcast notifications the
//! WebSocket layer forwards to connected UIs for live updates.

use std::path::Path;

use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;

/// True if a path is inside the local derived cache (`.wipe/.cache/...`).
fn is_cache_path(p: &Path) -> bool {
    p.components().any(|c| c.as_os_str() == ".cache")
}

/// Start watching `dir` recursively, sending `"changed"` on the broadcast channel
/// whenever anything under it changes. The returned watcher must be kept alive
/// for the duration of the process (dropping it stops watching).
///
/// Events that only touch the gitignored `.cache/` directory are ignored: reads
/// (e.g. a forum search) rewrite the derived index there, and treating that as a
/// change would feed back into the UI as a refetch and loop endlessly.
pub fn spawn(
    dir: &Path,
    tx: broadcast::Sender<String>,
) -> notify::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let only_cache =
                !event.paths.is_empty() && event.paths.iter().all(|p| is_cache_path(p));
            if only_cache {
                return;
            }
            // Ignore send errors: they only mean no UI is currently listening.
            let _ = tx.send("changed".to_string());
        }
    })?;
    watcher.watch(dir, RecursiveMode::Recursive)?;
    Ok(watcher)
}
