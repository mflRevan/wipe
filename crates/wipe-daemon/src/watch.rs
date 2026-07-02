//! Filesystem watching: turn `.wipe` changes into broadcast notifications the
//! WebSocket layer forwards to connected UIs for live updates.

use std::path::Path;

use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;

/// Start watching `dir` recursively, sending `"changed"` on the broadcast channel
/// whenever anything under it changes. The returned watcher must be kept alive
/// for the duration of the process (dropping it stops watching).
pub fn spawn(dir: &Path, tx: broadcast::Sender<String>) -> notify::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            // Ignore send errors: they only mean no UI is currently listening.
            let _ = tx.send("changed".to_string());
        }
    })?;
    watcher.watch(dir, RecursiveMode::Recursive)?;
    Ok(watcher)
}
