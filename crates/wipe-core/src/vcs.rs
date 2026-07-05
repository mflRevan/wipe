//! Version-control-agnostic identity resolution.
//!
//! wipe stores its board in a repo, but that repo isn't always git: teams on
//! Plastic SCM (Unity VCS), Mercurial, SVN, Fossil, or Jujutsu should still get
//! real attribution instead of "unknown". This module detects the VCS in use and
//! asks it who the current user is, shelling out to each tool best-effort (a
//! missing tool or non-repo simply yields `None`, never an error).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

/// Hard cap on any VCS probe. A hung tool (e.g. Plastic's `cm whoami` against an
/// unreachable server) must never block a `wipe` command or a daemon request.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// Cap on history walked when discovering authors, so large repos stay fast.
const AUTHOR_SCAN_LIMIT: &str = "2000";

/// A recognized version-control system hosting the repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vcs {
    Git,
    /// Plastic SCM / Unity Version Control (`cm`).
    Plastic,
    Mercurial,
    Svn,
    Fossil,
    Jujutsu,
    /// No recognized VCS at or above the path.
    None,
}

impl Vcs {
    /// Short, stable machine name (e.g. for `--json` / diagnostics).
    pub fn name(self) -> &'static str {
        match self {
            Vcs::Git => "git",
            Vcs::Plastic => "plastic",
            Vcs::Mercurial => "mercurial",
            Vcs::Svn => "svn",
            Vcs::Fossil => "fossil",
            Vcs::Jujutsu => "jujutsu",
            Vcs::None => "none",
        }
    }
}

/// Walk up from `start` looking for a VCS marker. The first match wins; `.git`
/// and `.plastic` are checked together at each level so a nested layout resolves
/// to the closest enclosing workspace.
pub fn detect(start: &Path) -> Vcs {
    let abs = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let mut cur: Option<PathBuf> = Some(abs);
    while let Some(dir) = cur {
        // Marker directories.
        for (marker, vcs) in [
            (".git", Vcs::Git),
            (".plastic", Vcs::Plastic),
            (".hg", Vcs::Mercurial),
            (".svn", Vcs::Svn),
            (".jj", Vcs::Jujutsu),
        ] {
            if dir.join(marker).exists() {
                return vcs;
            }
        }
        // Fossil uses a checkout marker file rather than a directory.
        if dir.join(".fslckout").exists() || dir.join("_FOSSIL_").exists() {
            return Vcs::Fossil;
        }
        cur = dir.parent().map(Path::to_path_buf);
    }
    Vcs::None
}

/// The current user's identity as reported by the repo's VCS (`Name <email>` when
/// available), or `None` if there's no VCS, the tool is missing, or it's unset.
pub fn identity(start: &Path) -> Option<String> {
    match detect(start) {
        Vcs::Git => git_identity(start),
        Vcs::Plastic => plastic_identity(start),
        Vcs::Mercurial => hg_identity(start),
        Vcs::Fossil => fossil_identity(start),
        Vcs::Jujutsu => jj_identity(start),
        // SVN has no reliable "current user" query; fall back to the OS user.
        Vcs::Svn | Vcs::None => None,
    }
}

/// Distinct historical authors as `(name, email)`, best-effort. Only git and
/// Mercurial expose this cheaply; others return an empty list.
pub fn authors(start: &Path) -> Vec<(String, String)> {
    match detect(start) {
        Vcs::Git => git_authors(start),
        Vcs::Mercurial => hg_authors(start),
        _ => Vec::new(),
    }
}

/// The OS account name (`$USERNAME` on Windows, `$USER` elsewhere), a last-ditch
/// identity when no VCS can supply one.
pub fn system_user() -> Option<String> {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// --- per-VCS implementations ----------------------------------------------

/// Strip Windows' `\\?\` verbatim prefix, which some CLIs (git `-C`) reject.
/// Handles UNC verbatim paths (`\\?\UNC\server\share` → `\\server\share`) so repos
/// on network shares still resolve.
pub(crate) fn plain(root: &Path) -> PathBuf {
    let s = root.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => root.to_path_buf(),
    }
}

/// Run a prepared command with a hard timeout, returning trimmed stdout on success.
/// On timeout (or spawn/exec failure) returns `None`; the abandoned child is left to
/// exit on its own rather than blocking us - identity is best-effort.
fn run_timed(mut cmd: Command) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(cmd.output());
    });
    match rx.recv_timeout(PROBE_TIMEOUT) {
        Ok(Ok(out)) => finish(out),
        _ => None,
    }
}

/// Run `program args...` in `dir` (via `-C`), returning trimmed stdout on success.
fn output_in(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.arg("-C").arg(plain(dir)).args(args);
    run_timed(cmd)
}

/// Run a command that takes its working directory via `current_dir` (tools
/// without a `-C` flag), returning trimmed stdout on success.
fn output_cwd(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.current_dir(plain(dir)).args(args);
    run_timed(cmd)
}

fn finish(out: std::process::Output) -> Option<String> {
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn combine(name: Option<String>, email: Option<String>) -> Option<String> {
    match (name, email) {
        (Some(n), Some(e)) => Some(format!("{n} <{e}>")),
        (Some(n), None) => Some(n),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}

fn git_identity(root: &Path) -> Option<String> {
    let name = output_in(root, "git", &["config", "user.name"]);
    let email = output_in(root, "git", &["config", "user.email"]);
    combine(name, email)
}

fn git_authors(root: &Path) -> Vec<(String, String)> {
    let Some(out) = output_in(
        root,
        "git",
        &[
            "--no-pager",
            "log",
            "-n",
            AUTHOR_SCAN_LIMIT,
            "--format=%an\t%ae",
        ],
    ) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut authors = Vec::new();
    for line in out.lines() {
        if let Some((name, email)) = line.split_once('\t') {
            if seen.insert(email.to_string()) {
                authors.push((name.to_string(), email.to_string()));
            }
        }
    }
    authors
}

fn plastic_identity(root: &Path) -> Option<String> {
    // `cm whoami` prints the current Plastic user (often an email-like seid).
    output_cwd(root, "cm", &["whoami"])
}

fn hg_identity(root: &Path) -> Option<String> {
    // `hg config ui.username` yields a ready-made "Name <email>".
    output_in(root, "hg", &["config", "ui.username"])
}

fn hg_authors(root: &Path) -> Vec<(String, String)> {
    let Some(out) = output_in(
        root,
        "hg",
        &["log", "-l", AUTHOR_SCAN_LIMIT, "--template", "{author}\n"],
    ) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut authors = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // "Name <email>" → split; otherwise treat the whole thing as the name.
        let (name, email) = match (line.find('<'), line.find('>')) {
            (Some(a), Some(b)) if b > a => {
                (line[..a].trim().to_string(), line[a + 1..b].to_string())
            }
            _ => (line.to_string(), line.to_string()),
        };
        if seen.insert(email.clone()) {
            authors.push((name, email));
        }
    }
    authors
}

fn fossil_identity(root: &Path) -> Option<String> {
    output_cwd(root, "fossil", &["user", "default"])
}

fn jj_identity(root: &Path) -> Option<String> {
    let name = output_cwd(root, "jj", &["config", "get", "user.name"]);
    let email = output_cwd(root, "jj", &["config", "get", "user.email"]);
    combine(name, email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_git_and_resolves_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap();
        };
        git(&["init", "-q"]);
        git(&["config", "user.name", "Ada Lovelace"]);
        git(&["config", "user.email", "ada@example.com"]);

        assert_eq!(detect(root), Vcs::Git);
        assert_eq!(
            identity(root).as_deref(),
            Some("Ada Lovelace <ada@example.com>")
        );
    }

    #[test]
    fn no_vcs_yields_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect(dir.path()), Vcs::None);
        assert_eq!(identity(dir.path()), None);
    }

    #[test]
    fn vcs_names_are_stable() {
        assert_eq!(Vcs::Plastic.name(), "plastic");
        assert_eq!(Vcs::Git.name(), "git");
        assert_eq!(Vcs::None.name(), "none");
    }
}
