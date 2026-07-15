//! User-global configuration (`<config>/wipe/config.json`).
//!
//! Distinct from a board's `settings.json` (which is per-project and git-tracked),
//! this file holds the *defaults* a user picks once during onboarding - preferred
//! port, exposure, whether to auto-serve, how much starter content a new board
//! gets, where to install the agent skill, and UI styling - so later
//! `wipe init` / `wipe serve` runs don't have to ask again.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{Exposure, Starter};

/// Machine-wide user preferences. Every field is optional; an absent field means
/// "use the built-in default".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default daemon port for new boards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_port: Option<u16>,
    /// Default exposure for new boards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_expose: Option<Exposure>,
    /// Default: shut the daemon down when idle (no overhead when not viewed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autoserve: Option<bool>,
    /// Default idle timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout_secs: Option<u64>,
    /// Start the wipe UI daemon automatically at login (an always-on, lightweight
    /// viewer). Backed by a per-OS login entry managed by the CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,
    /// How much content a fresh board is seeded with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starter: Option<Starter>,
    /// Preferred agent-skill install convention: `claude` or `agents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_target: Option<String>,
    /// Whether to install the skill user-globally (vs project-scoped) by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_global: Option<bool>,
    /// Preferred UI accent color (token or hex), surfaced to the board UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_accent: Option<String>,
    /// Preferred UI theme: `light`, `dark`, or `system`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_theme: Option<String>,
    /// Fallback identity used when a project's VCS reports no user (mandatory in
    /// practice: onboarding sets it, defaulting to `human`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_identity: Option<String>,
    /// Always attribute actions to [`default_identity`] instead of the VCS-reported
    /// user, even when the VCS does report one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_default_identity: Option<bool>,
    /// Directories scanned for `.wipe` boards (so serving surfaces every board you
    /// have locally). Empty/absent means "the user's home directory".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_roots: Option<Vec<String>>,
    /// How many days a deleted ticket is kept in the (gitignored) trash before it
    /// is permanently purged and can no longer be restored. Absent means the
    /// built-in default (7). `0` disables the trash entirely (immediate purge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_retention_days: Option<u64>,
}

/// The built-in default trash retention window, in days.
pub const DEFAULT_TRASH_RETENTION_DAYS: u64 = 7;

impl GlobalConfig {
    /// The effective trash retention window in days (falls back to the built-in
    /// default when unset).
    pub fn trash_retention_days(&self) -> u64 {
        self.trash_retention_days
            .unwrap_or(DEFAULT_TRASH_RETENTION_DAYS)
    }
}

impl GlobalConfig {
    /// Path to `config.json`. Honors `$WIPE_CONFIG_DIR` (useful for isolating
    /// tests and for pinning config in CI), else the user's platform config dir.
    pub fn path() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("WIPE_CONFIG_DIR") {
            if !dir.trim().is_empty() {
                return Some(PathBuf::from(dir).join("config.json"));
            }
        }
        directories::ProjectDirs::from("dev", "wipe", "wipe")
            .map(|d| d.config_dir().join("config.json"))
    }

    /// Load the config, returning defaults if the file is missing or unreadable.
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Persist the config (pretty JSON + trailing newline). Best-effort: creates
    /// the config directory if needed.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(path) = Self::path() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
            s.push('\n');
            // Write-then-rename so a concurrent reader (or a racing writer) never
            // sees a truncated file.
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, s)?;
            std::fs::rename(&tmp, &path)?;
        }
        Ok(())
    }
}
