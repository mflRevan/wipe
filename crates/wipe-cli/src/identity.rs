//! Resolve the acting identity for authored actions (comments, assignments).
//!
//! Resolution order: explicit `--author` → `$WIPE_AUTHOR` → git `user.name`/
//! `user.email` → `"unknown"`. Ties authorship to git so attribution is
//! consistent with commit history.

use std::process::Command;

/// Resolve the author identity, given an optional explicit override.
pub fn resolve(explicit: Option<String>) -> String {
    if let Some(a) = explicit.filter(|s| !s.trim().is_empty()) {
        return a;
    }
    if let Ok(a) = std::env::var("WIPE_AUTHOR") {
        if !a.trim().is_empty() {
            return a;
        }
    }
    match (git_config("user.name"), git_config("user.email")) {
        (Some(name), Some(email)) => format!("{name} <{email}>"),
        (Some(name), None) => name,
        (None, Some(email)) => email,
        (None, None) => "unknown".to_string(),
    }
}

/// Read a value from git config, or `None` if unavailable/empty.
fn git_config(key: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["config", "--get", key])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let val = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// Whether git is available on PATH (used by `wipe doctor`).
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
