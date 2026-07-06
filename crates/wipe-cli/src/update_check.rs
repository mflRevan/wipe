//! Best-effort, once-a-day "a newer version is available" notice.
//!
//! On any command we read a small cached state file; at most once every 24h we ask
//! the npm registry for the latest published version and cache it. If the cached
//! latest is newer than this build, we print a short notice to **stderr** (never
//! stdout, so `--json` output stays clean). Everything here is best-effort: offline
//! or slow networks never block or fail a command, and `WIPE_NO_UPDATE_CHECK`
//! disables it entirely.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Published npm package name (its `latest` dist-tag is the source of truth).
const PKG: &str = "@mflrevan/wipe";
const COOLDOWN_HOURS: i64 = 24;

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_check: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest: Option<String>,
}

fn state_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WIPE_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir).join("update-check.json"));
        }
    }
    directories::ProjectDirs::from("dev", "wipe", "wipe")
        .map(|d| d.config_dir().join("update-check.json"))
}

fn load() -> State {
    state_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(state: &State) {
    if let Some(path) = state_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut s) = serde_json::to_string_pretty(state) {
            s.push('\n');
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}

/// Ask the npm registry for the latest published version, with a hard wall-clock
/// bound. ureq's connect/read timeouts don't cover DNS resolution, so we run the
/// whole request on a thread and give up after 2s regardless - the check must never
/// noticeably delay a command.
fn fetch_latest() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_latest_inner());
    });
    rx.recv_timeout(Duration::from_secs(2)).ok().flatten()
}

fn fetch_latest_inner() -> Option<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(1500))
        .timeout_read(Duration::from_millis(1500))
        .build();
    let url = format!("https://registry.npmjs.org/{PKG}/latest");
    let body = agent.get(&url).call().ok()?.into_string().ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    v.get("version")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

/// Parse a `major.minor.patch` prefix, ignoring any `-pre`/`+build` suffix.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().split(['-', '+']).next().unwrap_or("");
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// Whether `latest` is a strictly newer version than `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Run the check (best-effort). `current` is this build's version.
pub fn run(current: &str) {
    if std::env::var_os("WIPE_NO_UPDATE_CHECK").is_some() {
        return;
    }
    let mut state = load();
    let due = state
        .last_check
        .map(|t| Utc::now().signed_duration_since(t).num_hours() >= COOLDOWN_HOURS)
        .unwrap_or(true);
    if due {
        // Stamp the check now so an offline run doesn't retry every command; the
        // next attempt is a full 24h out, per the cooldown.
        state.last_check = Some(Utc::now());
        if let Some(v) = fetch_latest() {
            state.latest = Some(v);
        }
        save(&state);
    }
    if let Some(latest) = &state.latest {
        if is_newer(latest, current) {
            eprintln!("wipe: a newer version is available: {latest} (you have {current})");
            eprintln!("      update: npm i -g {PKG}   ·   or: cargo install wipe-cli");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.3.3", "0.3.2"));
        assert!(is_newer("0.4.0", "0.3.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.3.2", "0.3.2"));
        assert!(!is_newer("0.3.1", "0.3.2"));
        // Pre-release/build suffixes are ignored for the comparison.
        assert!(!is_newer("0.3.2-rc1", "0.3.2"));
        assert!(is_newer("0.3.3-rc1", "0.3.2"));
        // Garbage never triggers a false "newer".
        assert!(!is_newer("garbage", "0.3.2"));
    }
}
