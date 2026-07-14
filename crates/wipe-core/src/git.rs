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
    let mut args: Vec<String> = vec![
        "--no-pager".into(),
        "log".into(),
        format,
        "--no-color".into(),
    ];
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
    // `rev:./path` resolves relative to git's cwd (our `-C root`), whereas a bare
    // `rev:path` is relative to the REPO root - which breaks for a board nested in
    // a subdirectory of the repo (e.g. a monorepo's `apps/foo/.wipe`). Forward
    // slashes work on all platforms for git pathspecs.
    let spec = format!("{rev}:./{}", relpath.replace('\\', "/"));
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

/// Turn an identity string into a git `(name, email)` pair. Accepts the usual
/// `Name <email>` form; a bare agent id (e.g. `claude-dev`) gets a stable,
/// synthetic `<id>@wipe.local` address so `git` accepts the attribution.
fn author_ident(author: &str) -> (String, String) {
    if let Some(open) = author.rfind('<') {
        if let Some(rel_close) = author[open..].find('>') {
            let name = author[..open].trim();
            let email = author[open + 1..open + rel_close].trim();
            if !email.is_empty() {
                let name = if name.is_empty() { email } else { name };
                return (name.to_string(), email.to_string());
            }
        }
    }
    let slug: String = author
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let slug = if slug.trim_matches('-').is_empty() {
        "agent".to_string()
    } else {
        slug
    };
    (author.to_string(), format!("{slug}@wipe.local"))
}

/// Stage the given `pathspecs` (relative to `root`) and create a commit
/// containing only those paths, attributed to `author` (both author and
/// committer). Returns the new commit's short hash, or `None` if there was
/// nothing to commit under the pathspecs. Other staged changes outside the
/// pathspecs are left untouched, so `.wipe/` commits stay atomic and isolated.
pub fn commit_paths(
    root: &Path,
    pathspecs: &[&str],
    message: &str,
    author: Option<&str>,
) -> Result<Option<String>> {
    // Stage (and thereby track any new files) under the requested paths only.
    let mut add: Vec<&str> = vec!["add", "--"];
    add.extend_from_slice(pathspecs);
    run(root, &add)?;

    // Is anything actually staged/changed under these paths?
    let mut status: Vec<&str> = vec!["status", "--porcelain", "--"];
    status.extend_from_slice(pathspecs);
    if run(root, &status)?.trim().is_empty() {
        return Ok(None);
    }

    let mut args: Vec<String> = Vec::new();
    if let Some(a) = author {
        let (name, email) = author_ident(a);
        args.push("-c".into());
        args.push(format!("user.name={name}"));
        args.push("-c".into());
        args.push(format!("user.email={email}"));
        args.push("commit".into());
        args.push(format!("--author={name} <{email}>"));
    } else {
        args.push("commit".into());
    }
    args.push("-m".into());
    args.push(message.to_string());
    // Limit the commit to the pathspecs so unrelated staged changes are excluded.
    args.push("--".into());
    for p in pathspecs {
        args.push((*p).to_string());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(root, &refs)?;

    // Report the freshly-created commit's short hash.
    let head = run(root, &["rev-parse", "--short", "HEAD"])?;
    Ok(Some(head.trim().to_string()))
}

/// Compute the git blob hash of `bytes` (identical to `git hash-object`).
pub fn blob_hash(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(format!("blob {}\0", bytes.len()).as_bytes());
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// All tracked files as `(blob_hash, repo-relative path)` pairs.
pub fn tracked_blobs(root: &Path) -> Result<Vec<(String, String)>> {
    let out = run(root, &["ls-files", "-s"])?;
    let mut blobs = Vec::new();
    for line in out.lines() {
        // Format: "<mode> <hash> <stage>\t<path>"
        if let Some((meta, path)) = line.split_once('\t') {
            let mut cols = meta.split_whitespace();
            let _mode = cols.next();
            if let Some(hash) = cols.next() {
                blobs.push((hash.to_string(), path.to_string()));
            }
        }
    }
    Ok(blobs)
}

/// Distinct commit authors as `(name, email)`, most-recent first.
pub fn authors(root: &Path) -> Result<Vec<(String, String)>> {
    let out = run(
        root,
        &["--no-pager", "log", &format!("--format=%an{FS}%ae")],
    )?;
    let mut seen = std::collections::HashSet::new();
    let mut authors = Vec::new();
    for line in out.lines() {
        if let Some((name, email)) = line.split_once(FS) {
            if seen.insert(email.to_string()) {
                authors.push((name.to_string(), email.to_string()));
            }
        }
    }
    Ok(authors)
}

/// The configured git identity for this repo (`Name <email>`), if set. Used to
/// attribute UI-driven changes in the activity timeline to the human at the keyboard.
pub fn config_identity(root: &Path) -> Option<String> {
    let get = |key: &str| {
        run(root, &["config", key])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    match (get("user.name"), get("user.email")) {
        (Some(name), Some(email)) => Some(format!("{name} <{email}>")),
        (Some(name), None) => Some(name),
        (None, Some(email)) => Some(email),
        (None, None) => None,
    }
}

/// A commit in the repository graph, with parent links and ref decorations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphCommit {
    /// Full commit hash.
    pub hash: String,
    /// Abbreviated hash.
    pub short: String,
    /// Parent commit hashes (2+ means a merge).
    pub parents: Vec<String>,
    /// Ref decorations pointing at this commit (branches, tags, HEAD).
    pub refs: Vec<String>,
    /// Author display name.
    pub author_name: String,
    /// Author date, ISO-8601.
    pub date: String,
    /// Commit subject.
    pub subject: String,
    /// Whether this commit changed the board (`.wipe/`) - a board "checkpoint".
    pub board: bool,
}

/// The commit graph across all branches (most recent first), with parent links,
/// ref decorations, and a flag marking commits that touched the board. Intended
/// for drawing a git-graph view of the board's history.
pub fn graph(root: &Path, limit: Option<usize>) -> Result<Vec<GraphCommit>> {
    // Hashes of commits that changed the board, so the UI can mark checkpoints.
    let board: std::collections::HashSet<String> = run(
        root,
        &["--no-pager", "log", "--all", "--format=%H", "--", ".wipe"],
    )
    .unwrap_or_default()
    .lines()
    .map(|s| s.trim().to_string())
    .collect();

    let format = format!("--format=%H{FS}%h{FS}%P{FS}%D{FS}%an{FS}%aI{FS}%s{RS}");
    let mut args: Vec<String> = vec![
        "--no-pager".into(),
        "log".into(),
        "--all".into(),
        "--date-order".into(),
        format,
        "--no-color".into(),
    ];
    if let Some(l) = limit {
        args.push("-n".into());
        args.push(l.to_string());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run(root, &refs)?;
    Ok(out
        .split(RS)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .filter_map(|record| {
            let mut f = record.split(FS);
            let hash = f.next()?.to_string();
            let short = f.next()?.to_string();
            let parents = f
                .next()?
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let refs = f
                .next()
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let author_name = f.next().unwrap_or("").to_string();
            let date = f.next().unwrap_or("").to_string();
            let subject = f.next().unwrap_or("").to_string();
            let board = board.contains(&hash);
            Some(GraphCommit {
                hash,
                short,
                parents,
                refs,
                author_name,
                date,
                subject,
                board,
            })
        })
        .collect())
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

/// Strip Windows' `\\?\` verbatim prefix, which `git -C` does not accept (UNC-aware).
fn plain(root: &Path) -> std::path::PathBuf {
    crate::vcs::plain(root)
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
        Err(Error::msg(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
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
