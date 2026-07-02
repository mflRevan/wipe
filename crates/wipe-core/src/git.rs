//! Lightweight git integration by shelling out to the `git` CLI.
//!
//! wipe is git-native, so `git` is always present. Rather than pull in a heavy
//! libgit2/gitoxide dependency (and its native build), we invoke `git` for the
//! few things the UI needs: the commit history of the board, and the contents of
//! a tracked file at a past commit (used for the board-rewind feature).

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::error::{Error, Result};

/// A single commit's metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitInfo {
    /// Full commit hash.
    pub hash: String,
    /// Abbreviated hash.
    pub short: String,
    /// Author display name.
    pub author_name: String,
    /// Author email.
    pub author_email: String,
    /// Author date, ISO-8601 / RFC-3339.
    pub date: String,
    /// Commit subject line.
    pub subject: String,
}

// Field/record separators unlikely to appear in commit metadata.
const FS: char = '\u{1f}';
const RS: char = '\u{1e}';

/// Whether `path` is inside a git work tree.
pub fn is_repo(root: &Path) -> bool {
    run(root, &["rev-parse", "--is-inside-work-tree"])
        .map(|o| o.trim() == "true")
        .unwrap_or(false)
}

/// Return the commit history, most recent first. If `pathspec` is given, only
/// commits touching that path are returned. `limit` caps the number of commits.
pub fn log(root: &Path, pathspec: Option<&str>, limit: Option<usize>) -> Result<Vec<CommitInfo>> {
    let format = format!("--format=%H{FS}%h{FS}%an{FS}%ae{FS}%aI{FS}%s{RS}");
    let mut args: Vec<String> =
        vec!["--no-pager".into(), "log".into(), format, "--no-color".into()];
    if let Some(l) = limit {
        args.push("-n".into());
        args.push(l.to_string());
    }
    if let Some(p) = pathspec {
        args.push("--".into());
        args.push(p.to_string());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run(root, &refs)?;
    Ok(parse_log(&out))
}

/// Read the contents of a tracked file as of a specific commit/ref. Returns
/// `None` if the file did not exist at that revision.
pub fn file_at_commit(root: &Path, rev: &str, relpath: &str) -> Result<Option<String>> {
    // Forward slashes work on all platforms for git pathspecs.
    let spec = format!("{rev}:{}", relpath.replace('\\', "/"));
    match run(root, &["--no-pager", "show", &spec]) {
        Ok(s) => Ok(Some(s)),
        // A non-zero exit here means "path not present at rev", not a hard error.
        Err(Error::Message(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The most recent commit that touched `relpath`, if any (for attribution).
pub fn last_change(root: &Path, relpath: &str) -> Result<Option<CommitInfo>> {
    Ok(log(root, Some(relpath), Some(1))?.into_iter().next())
}

fn parse_log(out: &str) -> Vec<CommitInfo> {
    out.split(RS)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .filter_map(|record| {
            let mut f = record.split(FS);
            Some(CommitInfo {
                hash: f.next()?.to_string(),
                short: f.next()?.to_string(),
                author_name: f.next()?.to_string(),
                author_email: f.next()?.to_string(),
                date: f.next()?.to_string(),
                subject: f.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Strip Windows' `\\?\` verbatim prefix, which `git -C` does not accept.
fn plain(root: &Path) -> std::path::PathBuf {
    let s = root.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) => std::path::PathBuf::from(rest),
        None => root.to_path_buf(),
    }
}

/// Run a git command in `root`, returning stdout on success or an
/// [`Error::Message`] carrying stderr on failure.
fn run(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(plain(root))
        .args(args)
        .output()
        .map_err(|e| Error::msg(format!("failed to run git: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(Error::msg(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    #[test]
    fn log_and_show_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.email", "t@example.com"]);
        git(root, &["config", "user.name", "Tester"]);
        std::fs::write(root.join("a.txt"), "v1\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "first commit"]);

        assert!(is_repo(root));
        let history = log(root, None, None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].subject, "first commit");
        assert_eq!(history[0].author_email, "t@example.com");

        let head = &history[0].hash;
        let content = file_at_commit(root, head, "a.txt").unwrap();
        assert_eq!(content.as_deref(), Some("v1\n"));

        // A path that never existed yields None, not an error.
        assert_eq!(file_at_commit(root, head, "missing.txt").unwrap(), None);
    }

    #[test]
    fn non_repo_reports_false() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_repo(dir.path()));
    }
}
