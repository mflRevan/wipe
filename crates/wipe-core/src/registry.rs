//! A machine-wide registry of known wipe projects, plus a filesystem scanner.
//!
//! Because collaboration is git-only, there's no server that "knows" your
//! projects. The daemon records every board it serves here so the UI can list and
//! switch between them - but a board cloned onto a fresh machine has never been
//! served, so it wouldn't appear. [`scan`] closes that gap by walking the disk for
//! `.wipe` directories and registering whatever it finds, so serving from anywhere
//! surfaces every board you have locally.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Store;

/// One registered project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectEntry {
    /// Absolute path to the project root (the parent of `.wipe`).
    pub path: String,
    /// Board name, resolved when listed (best-effort).
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    projects: Vec<String>,
}

/// Directory names never worth descending into while scanning for boards.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".hg",
    ".svn",
    ".plastic",
    ".jj",
    ".cache",
    "dist",
    "build",
    "out",
    ".svelte-kit",
    ".next",
    ".venv",
    "venv",
    "vendor",
    ".gradle",
    ".idea",
    ".vs",
    "Library", // Unity's generated folder (huge)
    "Temp",
];

/// Path to the registry JSON. Honors `$WIPE_CONFIG_DIR` (for test isolation and
/// pinning), else the user's platform config dir.
fn registry_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WIPE_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir).join("projects.json"));
        }
    }
    directories::ProjectDirs::from("dev", "wipe", "wipe")
        .map(|d| d.config_dir().join("projects.json"))
}

fn load() -> RegistryFile {
    registry_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(reg: &RegistryFile) {
    if let Some(path) = registry_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut s) = serde_json::to_string_pretty(reg) {
            s.push('\n');
            // Write-then-rename: a concurrent reader never sees a half-written file.
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}

/// Canonical registry key for a project root.
fn key_for(root: &Path) -> String {
    std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string()
}

/// Record a project root in the registry (idempotent). Best-effort: persistence
/// failures are ignored so serving never breaks over a registry issue. Returns
/// true if it was newly added.
pub fn register(root: &Path) -> bool {
    let key = key_for(root);
    let mut reg = load();
    if reg.projects.iter().any(|p| p == &key) {
        return false;
    }
    reg.projects.push(key);
    reg.projects.sort();
    save(&reg);
    true
}

/// Remove any registered projects whose `.wipe` no longer exists on disk.
pub fn prune() {
    let mut reg = load();
    let before = reg.projects.len();
    reg.projects.retain(|p| Store::open(p).is_ok());
    if reg.projects.len() != before {
        save(&reg);
    }
}

/// List all registered projects that still have a `.wipe` board, annotating each
/// with its current board name.
pub fn list() -> Vec<ProjectEntry> {
    load()
        .projects
        .into_iter()
        .filter_map(|path| {
            let store = Store::open(&path).ok()?;
            let name = store.load_board().map(|b| b.name).unwrap_or_default();
            Some(ProjectEntry { path, name })
        })
        .collect()
}

/// Default roots to scan when none are configured: the user's home directory.
pub fn default_scan_roots() -> Vec<PathBuf> {
    directories::UserDirs::new()
        .map(|d| vec![d.home_dir().to_path_buf()])
        .unwrap_or_default()
}

/// Walk `roots` (to `max_depth` levels) for `.wipe` boards and register each one.
/// Returns the newly-registered roots. Heavy/generated directories are skipped and
/// a board directory is never descended into (boards don't nest). A visit cap
/// bounds the worst case on very large trees.
pub fn scan(roots: &[PathBuf], max_depth: usize) -> Vec<String> {
    let mut found = Vec::new();
    for root in roots {
        // Give each root its own visit budget so an earlier, huge root can't
        // starve later ones (e.g. the cwd appended after the home dir).
        let mut budget: usize = 40_000;
        scan_dir(root, max_depth, &mut budget, &mut found);
    }
    found
}

fn scan_dir(dir: &Path, depth_left: usize, budget: &mut usize, found: &mut Vec<String>) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;

    // A board here: register and stop descending (boards don't contain boards).
    if dir.join(crate::WIPE_DIR).is_dir() {
        if register(dir) {
            found.push(key_for(dir));
        }
        return;
    }
    if depth_left == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') && name != "." || SKIP_DIRS.iter().any(|s| s == &name) {
            // Skip dotfolders and known-heavy generated dirs (but a `.wipe` at this
            // level was already handled above).
            continue;
        }
        scan_dir(&entry.path(), depth_left - 1, budget, found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_nested_boards() {
        let tmp = tempfile::tempdir().unwrap();
        // Isolate the registry to this test.
        std::env::set_var("WIPE_CONFIG_DIR", tmp.path().join("cfg"));

        let a = tmp.path().join("proj-a");
        let b = tmp.path().join("nested").join("deep").join("proj-b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        Store::init(&a, "A", chrono::Utc::now()).unwrap();
        Store::init(&b, "B", chrono::Utc::now()).unwrap();
        // A board buried under a skipped dir must NOT be found.
        let hidden = tmp.path().join("node_modules").join("proj-c");
        std::fs::create_dir_all(&hidden).unwrap();
        Store::init(&hidden, "C", chrono::Utc::now()).unwrap();

        let found = scan(&[tmp.path().to_path_buf()], 8);
        let names: Vec<String> = list().into_iter().map(|p| p.name).collect();
        assert!(names.contains(&"A".to_string()), "found A");
        assert!(names.contains(&"B".to_string()), "found B (nested)");
        assert!(
            !names.contains(&"C".to_string()),
            "C under node_modules skipped"
        );
        assert_eq!(found.len(), 2);

        std::env::remove_var("WIPE_CONFIG_DIR");
    }
}
