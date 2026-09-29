//! Resolve the acting identity for authored actions (tickets, comments, forum
//! posts, assignments) - explicit, session-aware, and never defaulted.
//!
//! Resolution order:
//!   1. an explicit `--author` on the subcommand
//!   2. the global `--agentid` override for this invocation
//!   3. `$WIPE_AGENT` - a stable per-process agent identity
//!   4. the identity bound to this terminal / agent session (`wipe identity use`)
//!   5. `$WIPE_AUTHOR`
//!
//! There is deliberately **no fallback** (no VCS user, no board or global
//! default): a CLI write with none of the above is refused with guidance on how
//! to choose one. Silently attributing work to whoever `git config` names is how
//! agents end up posting as their human, or as each other.
//!
//! `$WIPE_AGENT` (3) ranks above the session binding (4) on purpose: it is a plain
//! env var, so it is per-process, inherited by child processes, and cannot be
//! overwritten by another agent. Sessions are keyed per terminal tab / agent
//! session (see [`session_key`]), so binding one never leaks into another.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use wipe_core::model::IdentityKind;
use wipe_core::{ops, Store};

/// The `--agentid` override for the current process, if any.
static OVERRIDE: OnceLock<Option<String>> = OnceLock::new();

/// Record the global `--agentid` value once, at startup.
pub fn set_override(agentid: Option<String>) {
    let _ = OVERRIDE.set(agentid.filter(|s| !s.trim().is_empty()));
}

fn override_id() -> Option<String> {
    OVERRIDE.get().cloned().flatten()
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `$WIPE_AGENT` - a stable per-process agent identity. Being an env var it is
/// inherited by children and cannot be stomped by a concurrent agent, which makes
/// it the reliable mechanism when several agents share one machine/worktree.
pub fn agent_env() -> Option<String> {
    env_nonempty("WIPE_AGENT")
}

/// Resolve the acting identity, or `None` when nothing explicit names one.
pub fn resolve_opt(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(override_id)
        .or_else(agent_env)
        .or_else(active)
        .or_else(|| env_nonempty("WIPE_AUTHOR"))
}

/// Resolve the acting identity or fail with guidance on choosing one.
pub fn resolve(explicit: Option<String>) -> anyhow::Result<String> {
    resolve_opt(explicit.as_deref()).ok_or_else(|| anyhow::anyhow!(missing_identity_message()))
}

/// Where [`resolve_opt`] gets the identity from, for `whoami` / `doctor`.
pub fn source(explicit: Option<&str>) -> &'static str {
    if explicit.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        "explicit --author"
    } else if override_id().is_some() {
        "--agentid override"
    } else if agent_env().is_some() {
        "$WIPE_AGENT (per-process)"
    } else if active().is_some() {
        "session (wipe identity use)"
    } else if env_nonempty("WIPE_AUTHOR").is_some() {
        "$WIPE_AUTHOR"
    } else {
        "none chosen"
    }
}

/// The refusal shown when a write runs without an identity: which identities
/// exist on this board, the ways to pick one, and - for agents - to ask their
/// user rather than guess.
pub fn missing_identity_message() -> String {
    let mut m = String::from(
        "no identity chosen for this session - wipe never attributes writes to a default \
         identity, so this command was not run.\n",
    );
    let known = Store::discover(".")
        .ok()
        .and_then(|s| ops::list_identities(&s).ok())
        .unwrap_or_default();
    if known.is_empty() {
        m.push_str("  this board has no identities yet.\n");
    } else {
        m.push_str("  identities on this board:\n");
        let width = known.iter().map(|i| i.id.len()).max().unwrap_or(0).min(40);
        for i in &known {
            let kind = match i.kind {
                IdentityKind::Agent => "agent",
                IdentityKind::Human => "human",
            };
            let name = if i.display_name != i.id {
                format!("  {}", i.display_name)
            } else {
                String::new()
            };
            m.push_str(&format!("    {:<width$}  {kind}{name}\n", i.id));
        }
    }
    m.push_str(
        "  choose one:\n    \
         wipe identity use <id>            bind it to this terminal / agent session\n    \
         wipe identity use <new-id> --agent --name \"Name\"   (or --human) to create one\n    \
         export WIPE_AGENT=<id>            per-process, race-free (scripts, parallel agents)\n    \
         wipe --agentid <id> <command>     for a single command\n  \
         agents: if none of these identities fits you, stop and ask your user which identity \
         to use - never guess, and never write as a human's identity.",
    );
    if session_key().is_none() {
        m.push_str(
            "\n  note: no terminal/agent session could be detected here, so `wipe identity use` \
             cannot bind one - use $WIPE_AGENT, --agentid, or set $WIPE_SESSION=<unique id>.",
        );
    }
    m
}

/// If identity signals disagree with the resolved one (a stomp / confusion
/// hazard), a short note naming them; otherwise `None`.
pub fn divergence() -> Option<String> {
    let resolved = resolve_opt(None)?;
    let mut others: Vec<String> = Vec::new();
    if let Some(a) = agent_env().filter(|a| *a != resolved) {
        others.push(format!("$WIPE_AGENT={a}"));
    }
    if let Some(a) = active().filter(|a| *a != resolved) {
        others.push(format!("session={a}"));
    }
    if let Some(a) = env_nonempty("WIPE_AUTHOR").filter(|a| *a != resolved) {
        others.push(format!("$WIPE_AUTHOR={a}"));
    }
    if others.is_empty() {
        None
    } else {
        Some(format!(
            "other identity signals disagree with the active one: {}. \
             a concurrent agent may attribute work to a different id - pin yours with $WIPE_AGENT",
            others.join(", ")
        ))
    }
}

// --- session key -------------------------------------------------------------

/// Env vars set by AI agent harnesses. Inside one, the surrounding *terminal's*
/// session markers belong to the human who launched the agent, so they must not
/// key the agent's identity (or the agent would write as that human).
const AGENT_HARNESS_MARKERS: &[&str] = &[
    "AI_AGENT",
    "CLAUDECODE",
    "GEMINI_CLI",
    "CODEX_SANDBOX",
    "CURSOR_AGENT",
];

/// Per-session ids exported by agent harnesses.
const AGENT_SESSION_VARS: &[&str] = &["CLAUDE_CODE_SESSION_ID"];

/// Per-tab / per-pane ids exported by terminal emulators and multiplexers.
const TERMINAL_SESSION_VARS: &[&str] = &[
    "WT_SESSION",
    "TERM_SESSION_ID",
    "ITERM_SESSION_ID",
    "TMUX_PANE",
    "STY",
    "WEZTERM_PANE",
    "KITTY_WINDOW_ID",
    "GNOME_TERMINAL_SCREEN",
    "KONSOLE_DBUS_SESSION",
];

/// The key identifying this terminal tab or agent session, which `wipe identity
/// use` binds an identity to. In order:
///
/// 1. `$WIPE_SESSION` (explicit, any environment);
/// 2. an agent harness's own session id (e.g. Claude Code's);
/// 3. inside an agent harness without one: `None` - never the human's terminal;
/// 4. the terminal's per-tab id (Windows Terminal, iTerm, tmux, ...);
/// 5. the long-lived process that launched wipe (the interactive shell), keyed by
///    pid and start time so a recycled pid never inherits an old identity.
///
/// A fresh terminal or agent session therefore never inherits anyone's identity.
pub fn session_key() -> Option<String> {
    if let Some(s) = env_nonempty("WIPE_SESSION") {
        return Some(format!("wipe:{s}"));
    }
    for v in AGENT_SESSION_VARS {
        if let Some(s) = env_nonempty(v) {
            return Some(format!("{}:{s}", v.to_ascii_lowercase()));
        }
    }
    if AGENT_HARNESS_MARKERS
        .iter()
        .any(|v| env_nonempty(v).is_some())
    {
        return None;
    }
    for v in TERMINAL_SESSION_VARS {
        if let Some(s) = env_nonempty(v) {
            let s = if *v == "TMUX_PANE" {
                // Pane ids repeat across tmux servers; qualify with the socket.
                format!("{}{s}", env_nonempty("TMUX").unwrap_or_default())
            } else {
                s
            };
            return Some(format!("{}:{s}", v.to_ascii_lowercase()));
        }
    }
    process_anchor()
}

/// Unix: the terminal session (`getsid`), qualified by the session leader's start
/// time where the OS exposes it (Linux), so a reused pid is a different session.
#[cfg(unix)]
fn process_anchor() -> Option<String> {
    // SAFETY: getsid(0) only reads the calling process's session id.
    let sid = unsafe { libc::getsid(0) };
    if sid <= 0 {
        return None;
    }
    let start = std::fs::read_to_string(format!("/proc/{sid}/stat"))
        .ok()
        .and_then(|s| {
            // Field 22 (starttime), counted after the parenthesized command name.
            let rest = s.get(s.rfind(')')? + 2..)?;
            rest.split_whitespace().nth(19).map(str::to_string)
        })
        .unwrap_or_default();
    Some(format!("sid:{sid}:{start}"))
}

/// Windows: the nearest ancestor that is not a transient wrapper (npm's node /
/// cmd shims), keyed by pid + creation time. If wipe was launched straight from
/// a terminal host or the desktop rather than from a shell, there is no session.
#[cfg(windows)]
fn process_anchor() -> Option<String> {
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const TRANSIENT: &[&str] = &["node.exe", "cmd.exe", "wipe.exe"];
    const HOSTS: &[&str] = &[
        "windowsterminal.exe",
        "openconsole.exe",
        "conhost.exe",
        "explorer.exe",
        "code.exe",
        "services.exe",
        "svchost.exe",
    ];

    // SAFETY: plain Win32 snapshot/iteration over a correctly sized entry; the
    // handle is closed before returning.
    let procs: HashMap<u32, (u32, String)> = unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut map = HashMap::new();
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(0);
            let name = String::from_utf16_lossy(&e.szExeFile[..len]).to_ascii_lowercase();
            map.insert(e.th32ProcessID, (e.th32ParentProcessID, name));
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        map
    };

    let mut pid = procs.get(&std::process::id())?.0;
    for _ in 0..8 {
        let (parent, name) = procs.get(&pid)?;
        if HOSTS.contains(&name.as_str()) {
            return None;
        }
        if !TRANSIENT.contains(&name.as_str()) {
            // SAFETY: query-only handle, closed right after reading the times.
            let created = unsafe {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if h.is_null() {
                    return None;
                }
                let zero = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                let (mut c, mut x, mut k, mut u) = (zero, zero, zero, zero);
                let ok = GetProcessTimes(h, &mut c, &mut x, &mut k, &mut u) != 0;
                CloseHandle(h);
                if !ok {
                    return None;
                }
                (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime)
            };
            return Some(format!("proc:{pid}:{created}"));
        }
        pid = *parent;
    }
    None
}

#[cfg(not(any(unix, windows)))]
fn process_anchor() -> Option<String> {
    None
}

// --- session store ---------------------------------------------------------

/// Session bindings unused for this long are pruned on the next write.
const SESSION_TTL_DAYS: i64 = 30;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Sessions {
    /// session key -> bound identity.
    #[serde(default)]
    active: BTreeMap<String, String>,
    /// session key -> when it was bound (for pruning).
    #[serde(default)]
    bound: BTreeMap<String, DateTime<Utc>>,
}

fn sessions_path() -> Option<PathBuf> {
    if let Some(dir) = env_nonempty("WIPE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("sessions.json"));
    }
    directories::ProjectDirs::from("dev", "wipe", "wipe")
        .map(|d| d.config_dir().join("sessions.json"))
}

fn load_sessions() -> Sessions {
    sessions_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_sessions(s: &mut Sessions) -> anyhow::Result<()> {
    let cutoff = Utc::now() - chrono::Duration::days(SESSION_TTL_DAYS);
    // Drop stale bindings, and legacy ones that predate timestamps (e.g. the
    // pre-0.4 shared "default" session that leaked into every terminal).
    let keep: Vec<String> = s
        .active
        .keys()
        .filter(|k| s.bound.get(*k).is_some_and(|t| *t > cutoff))
        .cloned()
        .collect();
    s.active.retain(|k, _| keep.contains(k));
    s.bound.retain(|k, _| keep.contains(k));
    if let Some(path) = sessions_path() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut json = serde_json::to_string_pretty(s).unwrap_or_default();
        json.push('\n');
        // Write-then-rename so a concurrent reader never sees a truncated file.
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(())
}

/// The identity bound to the current session, if any.
pub fn active() -> Option<String> {
    let key = session_key()?;
    let s = load_sessions();
    s.bound.get(&key)?; // unstamped (legacy) bindings don't count
    s.active
        .get(&key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Bind `author` to the current session.
pub fn set_active(author: &str) -> anyhow::Result<()> {
    let key = session_key().ok_or_else(|| {
        anyhow::anyhow!(
            "cannot detect a terminal or agent session to bind `{author}` to here.\n  \
             use `export WIPE_AGENT={author}` (per-process), pass `--agentid {author}` per \
             command, or set $WIPE_SESSION to a unique id for this session first."
        )
    })?;
    let mut s = load_sessions();
    s.active.insert(key.clone(), author.to_string());
    s.bound.insert(key, Utc::now());
    save_sessions(&mut s)
}

/// Unbind the current session's identity.
pub fn clear_active() -> anyhow::Result<bool> {
    let Some(key) = session_key() else {
        return Ok(false);
    };
    let mut s = load_sessions();
    let removed = s.active.remove(&key).is_some();
    s.bound.remove(&key);
    if removed {
        save_sessions(&mut s)?;
    }
    Ok(removed)
}

/// The shell snippet that pins this identity per-process via `$WIPE_AGENT`, so
/// tool-spawned `wipe` processes that don't share the session still inherit it.
pub fn export_hint(author: &str) -> String {
    // Git Bash / MSYS / Cygwin on Windows are POSIX shells.
    let posix_shell = std::env::var_os("MSYSTEM").is_some() || std::env::var_os("SHELL").is_some();
    if cfg!(windows) && !posix_shell {
        format!("$env:WIPE_AGENT = \"{author}\"")
    } else {
        format!("export WIPE_AGENT=\"{author}\"")
    }
}

/// Best-effort: ensure an agent identity exists in the current board's registry so
/// it appears (as an agent) in listings. Used when `--agentid` / `$WIPE_AGENT`
/// names a new agent on a write.
///
/// Insert-only: if the id already exists we do nothing - never rewriting the file
/// and never clobbering a display name set earlier via `wipe identity use --name`.
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
