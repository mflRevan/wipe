//! A machine-wide registry of known wipe projects.
//!
//! Because collaboration is git-only, there is no server that "knows" your
//! projects. Instead, every time the daemon serves a board it records that
//! project's root in a small JSON file under the user's config directory, so the
//! UI can list and switch between every board you have opened locally.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use wipe_core::Store;

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

/// Path to the registry JSON (`<config>/wipe/projects.json`).
fn registry_path() -> Option<PathBuf> {
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
            let _ = std::fs::write(path, s);
        }
    }
}

/// Record a project root in the registry (idempotent). Best-effort: failures to
/// persist are ignored so serving never breaks over a registry issue.
pub fn register(root: &Path) {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let key = canonical.display().to_string();
    let mut reg = load();
    if !reg.projects.iter().any(|p| p == &key) {
        reg.projects.push(key);
        reg.projects.sort();
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
