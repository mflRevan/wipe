//! `wipe update`: update this installation through the channel it came from.
//!
//! The running binary's location tells the channel apart: inside an npm package
//! (global or a project's `node_modules`), or in `CARGO_HOME/bin` written either
//! by the release installer (it leaves a receipt) or by `cargo install` (listed
//! in `.crates2.json`). The matching tool then does the update, so its own
//! bookkeeping stays correct and `npm uninstall` / `cargo uninstall` keep working.
//!
//! Other wipe processes running from the same installation (a `wipe tray`, a
//! `wipe serve`) would keep serving the old version - and on Windows they lock
//! the files the update replaces - so they are closed first, after asking.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use wipe_core::vcs;

use crate::args::UpdateArgs;
use crate::output::Out;
use crate::update_check;

const PKG: &str = "@mflrevan/wipe";
const RELEASES: &str = "https://github.com/mflRevan/wipe/releases";
const EXE: &str = if cfg!(windows) { "wipe.exe" } else { "wipe" };

/// How this copy of wipe was installed.
#[derive(Debug, PartialEq)]
enum Channel {
    /// `npm i -g` into `prefix`.
    NpmGlobal { prefix: PathBuf },
    /// A dependency of the npm project in `project`.
    NpmLocal { project: PathBuf },
    /// The release's shell / PowerShell installer.
    Installer,
    /// `cargo install wipe-cli` from crates.io.
    Cargo,
}

impl Channel {
    fn label(&self) -> &'static str {
        match self {
            Channel::NpmGlobal { .. } => "npm, global",
            Channel::NpmLocal { .. } => "npm, project",
            Channel::Installer => "release installer",
            Channel::Cargo => "cargo install",
        }
    }

    /// The update command, for display and for running. `version` pins the
    /// release when known (else the channel's latest).
    fn command(&self, version: Option<&str>, force: bool) -> (Command, String) {
        let tag = version.unwrap_or("latest");
        let (mut cmd, shown) = match self {
            Channel::NpmGlobal { prefix } => {
                let mut c = Command::new(npm());
                c.args(["install", "--global", "--prefix"])
                    .arg(prefix)
                    .arg(format!("{PKG}@{tag}"));
                let shown = format!(
                    "npm install -g --prefix \"{}\" {PKG}@{tag}",
                    prefix.display()
                );
                (c, shown)
            }
            Channel::NpmLocal { project } => {
                let mut c = Command::new(npm());
                c.args(["install", &format!("{PKG}@{tag}")])
                    .current_dir(project);
                let shown = format!("npm install {PKG}@{tag}  (in {})", project.display());
                (c, shown)
            }
            Channel::Installer => {
                let base = match version {
                    Some(v) => format!("{RELEASES}/download/v{v}"),
                    None => format!("{RELEASES}/latest/download"),
                };
                if cfg!(windows) {
                    let script = format!("irm {base}/wipe-cli-installer.ps1 | iex");
                    let mut c = Command::new("powershell");
                    c.args([
                        "-NoProfile",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-Command",
                        &script,
                    ]);
                    (c, format!("powershell -c \"{script}\""))
                } else {
                    let script = format!(
                        "curl --proto '=https' --tlsv1.2 -LsSf {base}/wipe-cli-installer.sh | sh"
                    );
                    let mut c = Command::new("sh");
                    c.args(["-c", &script]);
                    (c, script)
                }
            }
            Channel::Cargo => {
                let mut c = Command::new("cargo");
                c.args(["install", "wipe-cli", "--locked"]);
                if let Some(v) = version {
                    c.args(["--version", v]);
                }
                if force {
                    c.arg("--force");
                }
                let shown = format!(
                    "cargo install wipe-cli --locked{}{}",
                    version
                        .map(|v| format!(" --version {v}"))
                        .unwrap_or_default(),
                    if force { " --force" } else { "" }
                );
                (c, shown)
            }
        };
        // Already on PATH; the installer must not add it again.
        cmd.env("WIPE_CLI_NO_MODIFY_PATH", "1");
        (cmd, shown)
    }
}

/// npm is a batch file on Windows; Rust runs `npm.cmd` (with safe quoting) when
/// named exactly.
fn npm() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

/// An installation: its channel, the binary that stays on PATH afterwards, and
/// every binary the update replaces (other processes running them must stop).
struct Install {
    channel: Channel,
    binary: PathBuf,
    files: Vec<PathBuf>,
}

/// `wipe update`
pub fn run(out: &Out, a: UpdateArgs) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let exe = std::env::current_exe().context("cannot locate the running wipe binary")?;
    let exe = canonical(&exe);
    let install = detect(&exe).map_err(|why| anyhow!(why))?;
    cleanup_old(&exe);

    let latest = update_check::latest_version(Duration::from_secs(10));
    let behind = latest
        .as_deref()
        .is_some_and(|l| update_check::is_newer(l, current));
    let (mut cmd, shown) = install.channel.command(latest.as_deref(), a.force);
    let others = processes(&install.files);

    if a.check {
        let procs: Vec<_> = others
            .iter()
            .map(|p| json!({ "pid": p.pid, "command": p.cmd }))
            .collect();
        if out.json {
            out.json_value(&json!({
                "ok": true, "current": current, "latest": latest, "update": behind,
                "channel": install.channel.label(), "binary": install.binary,
                "command": shown, "running": procs,
            }));
        } else {
            println!("wipe {current} - installed via {}", install.channel.label());
            println!("  binary:  {}", install.binary.display());
            match &latest {
                Some(l) if behind => println!("  latest:  {l} (update available)"),
                Some(l) => println!("  latest:  {l} (up to date)"),
                None => println!("  latest:  unknown (registry unreachable)"),
            }
            println!("  update:  {shown}");
            for p in &others {
                println!("  running: pid {} - {}", p.pid, p.cmd);
            }
        }
        return Ok(());
    }

    match &latest {
        Some(l) if !behind && !a.force => {
            out.ok(
                format!("wipe {current} is up to date ({})", install.channel.label()),
                json!({ "ok": true, "updated": false, "current": current, "latest": l }),
            );
            return Ok(());
        }
        None if !a.force => bail!(
            "cannot reach the npm registry to look up the latest version; check your \
             connection, or run `{shown}` yourself"
        ),
        _ => {}
    }

    let mut restart_tray = false;
    if !others.is_empty() {
        let list: Vec<String> = others
            .iter()
            .map(|p| format!("  pid {} - {}", p.pid, p.cmd))
            .collect();
        eprintln!(
            "These wipe processes run from this installation and must close for the update:\n{}",
            list.join("\n")
        );
        if !a.yes && !confirm(out, "Close them and update now?") {
            bail!(
                "update cancelled - close them first (or rerun with --yes to close them for you)"
            );
        }
        restart_tray = others.iter().any(|p| p.cmd.contains(" tray"));
        close(&others, &install.files)?;
    }

    eprintln!("updating via {}: {shown}", install.channel.label());
    let aside = move_aside(&exe);
    if out.json {
        // Keep stdout to the one JSON result.
        cmd.stdout(Stdio::from(std::io::stderr()));
    }
    let status = cmd.status();
    let ok = matches!(&status, Ok(s) if s.success());
    if !ok {
        restore(aside, &exe);
        let reason = match status {
            Ok(s) => format!("exited with {s}"),
            Err(e) => format!("could not start: {e}"),
        };
        bail!("the update command {reason}; nothing changed. Command: {shown}");
    }

    // A tool that "succeeded" without writing a binary (nothing to do) must not
    // leave the install without one.
    if !exe.exists() {
        restore(aside, &exe);
    }
    let now = installed_version(&install.binary).unwrap_or_else(|| "?".into());
    if restart_tray {
        let _ = vcs::command(&install.binary).arg("tray").spawn();
    }
    out.ok(
        format!(
            "updated wipe {current} -> {now} via {}{}",
            install.channel.label(),
            if restart_tray {
                " (tray restarted)"
            } else {
                ""
            }
        ),
        json!({
            "ok": true, "updated": true, "from": current, "to": now,
            "channel": install.channel.label(), "restarted_tray": restart_tray,
        }),
    );
    Ok(())
}

/// Ask on an interactive terminal; never in `--json` or piped runs.
fn confirm(out: &Out, question: &str) -> bool {
    use std::io::IsTerminal;
    if out.json || !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return false;
    }
    inquire::Confirm::new(question)
        .with_default(true)
        .prompt()
        .unwrap_or(false)
}

// --- channel detection ------------------------------------------------------

fn detect(exe: &Path) -> std::result::Result<Install, String> {
    if let Some(found) = detect_npm(exe) {
        return Ok(found);
    }
    let unknown = || {
        format!(
            "can't tell how this wipe was installed ({}). Update it the way you installed it: \
             `npm i -g {PKG}`, `cargo install wipe-cli`, or the installer from {RELEASES}/latest",
            exe.display()
        )
    };
    let bin = exe.parent().ok_or_else(unknown)?;
    if bin.file_name().and_then(|n| n.to_str()) != Some("bin") {
        return Err(unknown());
    }
    let cargo_home = bin.parent().ok_or_else(unknown)?;
    let receipt = receipt_path().filter(|r| receipt_covers(r, exe));
    let crates = cargo_home.join(".crates2.json");
    let cargo = cargo_entry(&crates);
    let channel = match (receipt, cargo) {
        (Some(r), Some(_)) if mtime(&r) >= mtime(&crates) => Channel::Installer,
        (Some(_), None) => Channel::Installer,
        (_, Some(source)) if source.contains("registry+") => Channel::Cargo,
        (_, Some(source)) => {
            return Err(format!(
                "this wipe was built from source ({source}); rebuild it the same way \
                 (e.g. `cargo install --path crates/wipe-cli` in your checkout)"
            ))
        }
        (None, None) => return Err(unknown()),
    };
    Ok(Install {
        channel,
        binary: exe.to_path_buf(),
        files: vec![exe.to_path_buf()],
    })
}

/// npm installs keep the binary in `node_modules/@mflrevan/wipe/node_modules/
/// .bin_real/`; on Windows a global install also copies it next to npm's shims.
fn detect_npm(exe: &Path) -> Option<Install> {
    let in_package = exe.ancestors().find(|a| {
        a.file_name().is_some_and(|n| n == "wipe")
            && a.parent()
                .and_then(|p| p.file_name())
                .is_some_and(|n| n == "@mflrevan")
    });
    let beside_shims = exe
        .parent()
        .map(|d| d.join("node_modules").join("@mflrevan").join("wipe"))
        .filter(|p| cfg!(windows) && p.join("package.json").is_file());
    let pkg = in_package.map(Path::to_path_buf).or(beside_shims)?;
    let node_modules = pkg.parent()?.parent()?;
    if node_modules.file_name()? != "node_modules" {
        return None;
    }
    let root = node_modules.parent()?;
    let real = pkg.join("node_modules").join(".bin_real").join(EXE);
    let mut files = vec![real.clone()];

    if depends_on_wipe(&root.join("package.json")) {
        let binary = if in_package.is_some() {
            exe.to_path_buf()
        } else {
            real
        };
        return Some(Install {
            channel: Channel::NpmLocal {
                project: root.to_path_buf(),
            },
            binary,
            files,
        });
    }
    // Global: `<prefix>/node_modules` on Windows, `<prefix>/lib/node_modules` elsewhere.
    let prefix = if !cfg!(windows) && root.file_name().is_some_and(|n| n == "lib") {
        root.parent()?.to_path_buf()
    } else {
        root.to_path_buf()
    };
    let copy = prefix.join(EXE);
    let binary = if cfg!(windows) { copy.clone() } else { real };
    if cfg!(windows) {
        files.push(copy);
    }
    Some(Install {
        channel: Channel::NpmGlobal { prefix },
        binary,
        files,
    })
}

fn depends_on_wipe(package_json: &Path) -> bool {
    let Some(v) = std::fs::read(package_json)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
    else {
        return false;
    };
    ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .any(|k| v.get(k).and_then(|d| d.get(PKG)).is_some())
}

/// Where the release installer leaves its receipt (cargo-dist's layout).
fn receipt_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ if cfg!(windows) => PathBuf::from(std::env::var_os("LOCALAPPDATA")?),
        _ => directories::BaseDirs::new()?.home_dir().join(".config"),
    };
    let p = base.join("wipe-cli").join("wipe-cli-receipt.json");
    p.is_file().then_some(p)
}

/// Whether the receipt's install prefix holds `exe`.
fn receipt_covers(receipt: &Path, exe: &Path) -> bool {
    std::fs::read(receipt)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("install_prefix")?.as_str().map(PathBuf::from))
        .is_some_and(|prefix| starts_with(exe, &canonical(&prefix)))
}

/// The `.crates2.json` key's source for wipe-cli, e.g.
/// `wipe-cli 0.4.5 (registry+https://github.com/rust-lang/crates.io-index)`.
fn cargo_entry(crates: &Path) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(crates).ok()?).ok()?;
    v.get("installs")?
        .as_object()?
        .keys()
        .find(|k| k.starts_with("wipe-cli "))
        .cloned()
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn installed_version(binary: &Path) -> Option<String> {
    let out = vcs::command(binary).arg("--version").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.split_whitespace().nth(1).map(str::to_string)
}

// --- paths --------------------------------------------------------------------

fn canonical(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    vcs::plain(&c)
}

fn key(p: &Path) -> String {
    let s = canonical(p).to_string_lossy().into_owned();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

fn same(a: &Path, b: &Path) -> bool {
    key(a) == key(b)
}

fn starts_with(p: &Path, prefix: &Path) -> bool {
    let (p, prefix) = (key(p), key(prefix));
    p.strip_prefix(&prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '\\']))
}

// --- other processes ------------------------------------------------------------

struct Proc {
    pid: u32,
    cmd: String,
}

/// Other processes whose binary is one of `files`.
fn processes(files: &[PathBuf]) -> Vec<Proc> {
    let me = std::process::id();
    running()
        .into_iter()
        .filter(|(pid, exe, _)| *pid != me && files.iter().any(|f| same(exe, f)))
        .map(|(pid, _, cmd)| Proc { pid, cmd })
        .collect()
}

/// `(pid, binary, command line)` of every running wipe.
#[cfg(windows)]
fn running() -> Vec<(u32, PathBuf, String)> {
    let script = "Get-CimInstance Win32_Process -Filter \"Name='wipe.exe'\" | \
                  ForEach-Object { \"$($_.ProcessId)`t$($_.ExecutablePath)`t$($_.CommandLine)\" }";
    let Ok(out) = vcs::command("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut f = l.splitn(3, '\t');
            let pid = f.next()?.trim().parse().ok()?;
            let exe = PathBuf::from(f.next()?.trim());
            let cmd = command_line(f.next().unwrap_or("").trim());
            Some((pid, exe, cmd))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn running() -> Vec<(u32, PathBuf, String)> {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let exe = std::fs::read_link(e.path().join("exe")).ok()?;
            let exe = PathBuf::from(exe.to_string_lossy().trim_end_matches(" (deleted)"));
            let raw = std::fs::read(e.path().join("cmdline")).ok()?;
            let cmd = String::from_utf8_lossy(&raw).replace('\0', " ");
            Some((pid, exe, command_line(cmd.trim())))
        })
        .collect()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn running() -> Vec<(u32, PathBuf, String)> {
    Vec::new()
}

/// `"C:\...\wipe.exe" tray --foreground` -> `wipe tray --foreground`.
fn command_line(full: &str) -> String {
    let rest = if let Some(quoted) = full.strip_prefix('"') {
        quoted.split_once('"').map(|(_, r)| r).unwrap_or("")
    } else {
        full.split_once(' ').map(|(_, r)| r).unwrap_or("")
    };
    format!("wipe {}", rest.trim()).trim_end().to_string()
}

/// Stop `procs` and wait (briefly) until nothing runs from `files` any more.
fn close(procs: &[Proc], files: &[PathBuf]) -> Result<()> {
    for p in procs {
        terminate(p.pid);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = processes(files);
        if left.is_empty() {
            return Ok(());
        }
        if Instant::now() > deadline {
            let pids: Vec<String> = left.iter().map(|p| p.pid.to_string()).collect();
            bail!("these wipe processes did not close: {}", pids.join(", "));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(windows)]
fn terminate(pid: u32) {
    let _ = vcs::command("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output();
}

#[cfg(unix)]
fn terminate(pid: u32) {
    // SAFETY: plain signal to a pid we just listed.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
}

// --- the running binary (Windows) ---------------------------------------------

/// Windows won't overwrite or delete a running exe, but it will rename one.
/// Move this process's binary out of the way so the update can write its own
/// (into the temp dir, which also frees an npm package dir for removal).
#[cfg(windows)]
fn move_aside(exe: &Path) -> Option<PathBuf> {
    let stamp = chrono::Utc::now().timestamp_millis();
    let candidates = [
        std::env::temp_dir().join(format!("wipe-old-{stamp}.exe")),
        exe.with_file_name(format!("wipe.exe.old-{stamp}")),
    ];
    candidates
        .into_iter()
        .find(|to| std::fs::rename(exe, to).is_ok())
}

#[cfg(not(windows))]
fn move_aside(_exe: &Path) -> Option<PathBuf> {
    None
}

/// Undo [`move_aside`] after a failed update (if the update left nothing there).
fn restore(aside: Option<PathBuf>, exe: &Path) {
    if let Some(from) = aside {
        if !exe.exists() {
            let _ = std::fs::rename(from, exe);
        }
    }
}

/// Remove binaries earlier updates moved aside (best-effort; still-running ones stay).
fn cleanup_old(exe: &Path) {
    let dirs = [
        exe.parent().map(Path::to_path_buf),
        Some(std::env::temp_dir()),
    ];
    for dir in dirs.into_iter().flatten() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("wipe.exe.old-")
                || (name.starts_with("wipe-old-") && name.ends_with(".exe"))
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines_drop_the_binary_path() {
        assert_eq!(
            command_line(r#""C:\a b\wipe.exe" tray --foreground"#),
            "wipe tray --foreground"
        );
        assert_eq!(
            command_line("/usr/bin/wipe serve --port 7700"),
            "wipe serve --port 7700"
        );
        assert_eq!(command_line("wipe"), "wipe");
    }

    #[test]
    fn npm_installs_are_told_apart_by_the_project_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let pkg = root.join("node_modules").join("@mflrevan").join("wipe");
        let real = pkg.join("node_modules").join(".bin_real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(pkg.join("package.json"), "{}").unwrap();
        let exe = real.join(EXE);

        // No manifest naming wipe: a global install with `root` as the prefix.
        let global = detect_npm(&exe).unwrap();
        let expect_prefix = root.clone();
        assert_eq!(
            global.channel,
            Channel::NpmGlobal {
                prefix: expect_prefix
            }
        );

        // Windows global installs also copy the binary next to npm's shims.
        if cfg!(windows) {
            let copy = detect_npm(&root.join(EXE)).unwrap();
            assert!(same(&copy.binary, &root.join(EXE)));
        }

        std::fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"@mflrevan/wipe":"^0.4.0"}}"#,
        )
        .unwrap();
        let local = detect_npm(&exe).unwrap();
        assert_eq!(
            local.channel,
            Channel::NpmLocal {
                project: root.clone()
            }
        );
        assert!(same(&local.binary, &exe));

        // Not inside an npm package at all.
        assert!(detect_npm(&root.join("elsewhere").join(EXE)).is_none());
    }
}
