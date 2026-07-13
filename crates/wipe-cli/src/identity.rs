//! Resolve the acting identity for authored actions (tickets, comments, forum
//! posts, assignments) - VCS-agnostic and session-aware.
//!
//! Resolution order:
//!   1. an explicit `--author` on the subcommand
//!   2. the global `--agentid` override for this invocation
//!   3. `$WIPE_AGENT` - a stable per-terminal agent identity
//!   4. the identity bound to this terminal session (`wipe identity use`)
//!   5. `$WIPE_AUTHOR`
//!   6. the project's VCS user / board default / global default (via `wipe-core`)
//!
//! `$WIPE_AGENT` (3) ranks above the session file (4) on purpose: it is a plain
//! env var, so it is per-process, inherited by child processes, and CANNOT be
//! overwritten by another agent the way the shared `sessions.json` can. For
//! multiple agents sharing one machine/worktree, a harness that sets
//! `WIPE_AGENT=<id>` once per terminal gets race-free, correct attribution on every
//! command - no `wipe identity use` and no `--agentid` on each call. Sessions (keyed
//! by `$WIPE_SESSION`) remain for interactive single-agent use.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use wipe_core::model::IdentityKind;
use wipe_core::{ops, GlobalConfig, Store};

/// The `--agentid` override for the current process, if any.
static OVERRIDE: OnceLock<Option<String>> = OnceLock::new();

/// Record the global `--agentid` value once, at startup.
pub fn set_override(agentid: Option<String>) {
    let _ = OVERRIDE.set(agentid.filter(|s| !s.trim().is_empty()));
}

fn override_id() -> Option<String> {
    OVERRIDE.get().cloned().flatten()
}

/// `$WIPE_AGENT` - a stable per-terminal agent identity. Being an env var it is
/// per-process and inherited by children, so unlike the shared session file it
/// cannot be stomped by a concurrent agent. The reliable identity mechanism when
/// several agents share one machine/worktree.
pub fn agent_env() -> Option<String> {
    std::env::var("WIPE_AGENT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Resolve the author identity, given an optional explicit override.
pub fn resolve(explicit: Option<String>) -> String {
    if let Some(a) = explicit.filter(|s| !s.trim().is_empty()) {
        return a;
    }
    if let Some(a) = override_id() {
        return a;
    }
    if let Some(a) = agent_env() {
        return a;
    }
    if let Some(a) = active() {
        return a;
    }
    if let Ok(a) = std::env::var("WIPE_AUTHOR") {
        if !a.trim().is_empty() {
            return a;
        }
    }
    let store = Store::discover(".").ok();
    ops::resolve_identity(store.as_ref(), None)
}

/// A human-readable note on where `resolve` would get the identity from (for
/// `wipe identity whoami` / diagnostics).
pub fn source(explicit: Option<&str>) -> &'static str {
    // Matches ops::resolve_identity: when `prefer` is set and a default exists, the
    // default outranks the VCS.
    let prefer_default = {
        let g = GlobalConfig::load();
        g.prefer_default_identity.unwrap_or(false)
            && g.default_identity
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
    };
    if explicit.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        "explicit --author"
    } else if override_id().is_some() {
        "--agentid override"
    } else if agent_env().is_some() {
        "$WIPE_AGENT (per-terminal)"
    } else if active().is_some() {
        "session (wipe identity use)"
    } else if std::env::var("WIPE_AUTHOR")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        "$WIPE_AUTHOR"
    } else if prefer_default {
        "global default (preferred)"
    } else if Store::discover(".")
        .ok()
        .map(|s| wipe_core::vcs::identity(s.root()).is_some())
        .unwrap_or_else(|| wipe_core::vcs::identity(std::path::Path::new(".")).is_some())
    {
        "project VCS"
    } else {
        "default identity"
    }
}

// --- session store ---------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct Sessions {
    #[serde(default)]
    active: BTreeMap<String, String>,
}

fn sessions_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WIPE_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir).join("sessions.json"));
        }
    }
    directories::ProjectDirs::from("dev", "wipe", "wipe")
        .map(|d| d.config_dir().join("sessions.json"))
}

/// Key identifying this terminal session (`$WIPE_SESSION`, else `"default"`).
fn session_key() -> String {
    std::env::var("WIPE_SESSION")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

fn load_sessions() -> Sessions {
    sessions_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_sessions(s: &Sessions) -> anyhow::Result<()> {
    if let Some(path) = sessions_path() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut json = serde_json::to_string_pretty(s).unwrap_or_default();
        json.push('\n');
        // Write-then-rename so a concurrent reader never sees a truncated file.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(())
}

/// The identity bound to the current session, if any (ignoring blank values).
pub fn active() -> Option<String> {
    load_sessions()
        .active
        .get(&session_key())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Bind `author` to the current session.
pub fn set_active(author: &str) -> anyhow::Result<()> {
    let mut s = load_sessions();
    s.active.insert(session_key(), author.to_string());
    save_sessions(&s)
}

/// Unbind the current session's identity.
pub fn clear_active() -> anyhow::Result<bool> {
    let mut s = load_sessions();
    let removed = s.active.remove(&session_key()).is_some();
    if removed {
        save_sessions(&s)?;
    }
    Ok(removed)
}

/// The shell snippet an agent can `eval` to pin this identity for the session, so
/// even tools that spawn fresh `wipe` processes inherit it via `$WIPE_AUTHOR`.
pub fn export_hint(author: &str) -> String {
    if cfg!(windows) {
        format!("$env:WIPE_AUTHOR = \"{author}\"")
    } else {
        format!("export WIPE_AUTHOR=\"{author}\"")
    }
}

/// Best-effort: ensure an agent identity exists in the current board's registry so
/// it appears (as an agent) in listings. Used when `--agentid` names a new agent.
///
/// Insert-only: if the id already exists we do nothing - never rewriting the file
/// (so read-only commands don't dirty the repo) and never clobbering a display name
/// set earlier via `wipe identity use ... --name`.
pub fn ensure_registered(id: &str, name: Option<&str>, agent: bool) {
    let id = id.trim();
    if id.is_empty() {
        return;
    }
    if let Ok(store) = Store::discover(".") {
        if store
            .load_identities()
            .map(|list| list.iter().any(|i| i.id == id))
            .unwrap_or(false)
        {
            return; // already known - leave it (and the file) untouched
        }
        let kind = if agent {
            IdentityKind::Agent
        } else {
            IdentityKind::Human
        };
        let _ = ops::upsert_identity(&store, id, name.unwrap_or(id), Some(kind));
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

/// The default identity that will apply when nothing else supplies one, defaulting
/// to `human` (never empty), so attribution is never blank.
pub fn effective_default() -> String {
    GlobalConfig::load()
        .default_identity
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "human".to_string())
}
