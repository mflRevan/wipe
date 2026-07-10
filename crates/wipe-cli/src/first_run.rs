//! First-run onboarding offer.
//!
//! `cargo install` has no post-install hook and npm's `postinstall` runs in a
//! non-interactive context (often with stdin closed, or suppressed entirely), so
//! neither install channel can reliably prompt at install time. Both channels do,
//! however, drop the *same* `wipe` binary on disk - so we make the offer on the
//! first interactive run instead, which covers them all.
//!
//! The very first time `wipe` runs interactively on a machine that has no global
//! config yet, we offer to run the guided `wipe onboard` setup. It's best-effort
//! and one-shot: declining is remembered (in `first-run.json`, next to the config)
//! so we never nag, it's fully skipped for `--json`/piped/CI use, and
//! `WIPE_NO_ONBOARD_PROMPT` disables it entirely.

use std::io::IsTerminal;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use wipe_core::config::GlobalConfig;

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    /// Whether we've already made the first-run offer (accepted or declined).
    #[serde(default)]
    offered: bool,
}

fn state_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WIPE_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir).join("first-run.json"));
        }
    }
    directories::ProjectDirs::from("dev", "wipe", "wipe")
        .map(|d| d.config_dir().join("first-run.json"))
}

fn already_offered() -> bool {
    state_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<State>(&b).ok())
        .map(|s| s.offered)
        .unwrap_or(false)
}

fn mark_offered() {
    if let Some(path) = state_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut s) = serde_json::to_string_pretty(&State { offered: true }) {
            s.push('\n');
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}

/// Whether this looks like a fresh, interactive first run that should be offered
/// the guided setup: interactive TTY, no `--json`, no global config yet, not opted
/// out, and not already offered once.
pub fn should_offer(json: bool) -> bool {
    if json || std::env::var_os("WIPE_NO_ONBOARD_PROMPT").is_some() {
        return false;
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return false;
    }
    // A global config already exists → the user has set up (or configured) wipe.
    if GlobalConfig::path().map(|p| p.exists()).unwrap_or(false) {
        return false;
    }
    // We've already made the offer once → never ask again.
    !already_offered()
}

/// Prompt the user to run the guided setup now. Records that the offer was made
/// (so it never repeats) and returns whether the user accepted.
pub fn offer() -> bool {
    // Record first, so a Ctrl-C at the prompt doesn't re-offer on the next run.
    mark_offered();
    println!("Welcome to wipe! It looks like this is your first run on this machine.");
    let proceed = inquire::Confirm::new("Run the quick guided setup now?")
        .with_default(true)
        .with_help_message(
            "Sets machine-wide defaults (identity, UI port, autostart, styling). \
             Same as `wipe onboard`, which you can run anytime.",
        )
        .prompt()
        .unwrap_or(false);
    if !proceed {
        println!("No problem - run `wipe onboard` whenever you're ready.\n");
    }
    proceed
}
