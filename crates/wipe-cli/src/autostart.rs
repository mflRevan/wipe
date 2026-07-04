//! Best-effort OS login autostart for the always-on wipe UI daemon.
//!
//! Enabling writes a small per-user login entry that runs `wipe serve --idle 0`
//! (never auto-stops) at sign-in; disabling removes it. This is intentionally
//! lightweight and reversible - it never installs a system service or touches
//! anything outside the current user's own startup configuration. `enable`/
//! `disable` return a short human note describing what happened.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Absolute path to the `wipe` executable currently running (so the login entry
/// points at this exact install), falling back to the bare command name.
fn wipe_exe() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "wipe".to_string())
}

/// Whether a login entry is currently installed.
pub fn is_enabled() -> bool {
    imp::entry_path().map(|p| p.exists()).unwrap_or(false)
}

/// Install the login entry (idempotent).
pub fn enable() -> Result<String> {
    imp::enable()
}

/// Remove the login entry (idempotent - succeeds even if none exists).
pub fn disable() -> Result<String> {
    imp::disable()
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;

    pub fn entry_path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|appdata| {
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("Startup")
                .join("wipe-autoserve.cmd")
        })
    }

    pub fn enable() -> Result<String> {
        let path = entry_path().context("cannot locate the Startup folder (%APPDATA%)")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // `start "" /min` launches detached and minimized so login isn't blocked.
        let body = format!(
            "@echo off\r\nstart \"wipe\" /min \"{}\" serve --idle 0\r\n",
            wipe_exe()
        );
        std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
        Ok(format!("added a login entry at {}", path.display()))
    }

    pub fn disable() -> Result<String> {
        let path = entry_path().context("cannot locate the Startup folder (%APPDATA%)")?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok("removed the wipe login entry".to_string())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;

    pub fn entry_path() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("LaunchAgents")
                .join("dev.wipe.autoserve.plist")
        })
    }

    pub fn enable() -> Result<String> {
        let path = entry_path().context("cannot locate ~/Library/LaunchAgents")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>dev.wipe.autoserve</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>serve</string><string>--idle</string><string>0</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
            wipe_exe()
        );
        std::fs::write(&path, plist).with_context(|| format!("writing {}", path.display()))?;
        // Best-effort load so it starts this session too; ignore failures.
        let _ = std::process::Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&path)
            .status();
        Ok(format!("installed a LaunchAgent at {}", path.display()))
    }

    pub fn disable() -> Result<String> {
        let path = entry_path().context("cannot locate ~/Library/LaunchAgents")?;
        if path.exists() {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&path)
                .status();
            std::fs::remove_file(&path)?;
        }
        Ok("removed the wipe LaunchAgent".to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use super::*;

    pub fn entry_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(
            base.join("systemd")
                .join("user")
                .join("wipe-autoserve.service"),
        )
    }

    pub fn enable() -> Result<String> {
        let path = entry_path().context("cannot locate the systemd user unit directory")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let unit = format!(
            "[Unit]\nDescription=wipe UI daemon (always-on)\nAfter=default.target\n\n\
             [Service]\nExecStart={} serve --idle 0\nRestart=on-failure\n\n\
             [Install]\nWantedBy=default.target\n",
            wipe_exe()
        );
        std::fs::write(&path, unit).with_context(|| format!("writing {}", path.display()))?;
        // Best-effort enable+start; ignore failures (e.g. no systemd).
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", "wipe-autoserve.service"])
            .status();
        Ok(format!(
            "installed a systemd user unit at {}",
            path.display()
        ))
    }

    pub fn disable() -> Result<String> {
        let path = entry_path().context("cannot locate the systemd user unit directory")?;
        if path.exists() {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", "wipe-autoserve.service"])
                .status();
            std::fs::remove_file(&path)?;
        }
        Ok("removed the wipe systemd user unit".to_string())
    }
}
