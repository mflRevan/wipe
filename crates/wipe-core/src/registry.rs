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
    "AppData", // per-user app state on Windows (huge, never a project)
    "steamapps",
];

/// Operating-system trees at the top of a drive or volume that never hold
/// projects. Only skipped right below a scan root (a project folder deeper down
/// may legitimately be called e.g. `Windows`).
const ROOT_SKIP_DIRS: &[&str] = &[
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "Recovery",
    "PerfLogs",
    "System Volume Information",
    "Config.Msi",
    "MSOCache",
    "Intel",
    "AMD",
    "NVIDIA",
    // unix volumes / system roots
    "System",
    "Applications",
    "private",
    "proc",
    "sys",
    "dev",
    "usr",
    "lib",
    "bin",
    "sbin",
    "etc",
    "lost+found",
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

/// Default roots to scan when none are configured: the user's home directory,
/// then every local fixed drive / mounted volume - so boards on `D:\` or an
/// external data disk are found too. Network and removable drives are left out
/// (they can be slow or prompt), as is anything unreachable.
pub fn default_scan_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = directories::UserDirs::new()
        .map(|d| vec![d.home_dir().to_path_buf()])
        .unwrap_or_default();
    for v in local_volumes() {
        if !roots.contains(&v) {
            roots.push(v);
        }
    }
    roots
}

/// Root directories of this machine's local fixed drives (Windows: `C:\`,
/// `D:\`, ... with drive type "fixed").
#[cfg(windows)]
fn local_volumes() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    const DRIVE_FIXED: u32 = 3;
    // SAFETY: GetLogicalDrives takes no arguments; GetDriveTypeW reads a
    // NUL-terminated wide string we own for the duration of the call.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u32)
        .filter(|i| mask & (1 << i) != 0)
        .filter_map(|i| {
            let letter = (b'A' + i as u8) as char;
            let wide: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
            (unsafe { GetDriveTypeW(wide.as_ptr()) } == DRIVE_FIXED)
                .then(|| PathBuf::from(format!("{letter}:\\")))
        })
        .collect()
}

/// Mounted data volumes: `/Volumes/*` (macOS), `/mnt/*` and `/media/<user>/*` (Linux).
#[cfg(not(windows))]
fn local_volumes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut add_children = |dir: &Path| {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                // The boot volume is just a link back to `/`.
                if p.is_dir()
                    && std::fs::canonicalize(&p)
                        .map(|c| c != Path::new("/"))
                        .unwrap_or(false)
                {
                    out.push(p);
                }
            }
        }
    };
    add_children(Path::new("/Volumes"));
    add_children(Path::new("/mnt"));
    if let Ok(user) = std::env::var("USER") {
        add_children(&Path::new("/media").join(user));
    }
    out
}

/// Walk `roots` (to `max_depth` levels) for `.wipe` boards and register each one.
/// Returns the newly-registered roots. Heavy/generated directories are skipped and
/// a board directory is never descended into (boards don't nest). A visit cap
/// bounds the worst case on very large trees. A root nested inside another (your
/// home inside `C:\`) is walked once, as its own root.
pub fn scan(roots: &[PathBuf], max_depth: usize) -> Vec<String> {
    let mut found = Vec::new();
    let others: Vec<String> = roots.iter().map(|r| norm(&canon(r))).collect();
    for (i, root) in roots.iter().enumerate() {
        // Give each root its own visit budget so an earlier, huge root can't
        // starve later ones (e.g. a drive after the home dir).
        let mut ctx = Walk {
            budget: 40_000,
            found: &mut found,
            skip: others
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, p)| p.clone())
                .collect(),
        };
        scan_dir(root, max_depth, true, &mut ctx);
    }
    found
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// A comparable form of an absolute path without touching the disk: no Windows
/// verbatim prefix, no trailing separator, case-folded on Windows.
fn norm(p: &Path) -> String {
    let s = p.display().to_string();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.trim_end_matches(['/', '\\']);
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

struct Walk<'a> {
    budget: usize,
    found: &'a mut Vec<String>,
    /// Other scan roots (normalized): not re-walked from here.
    skip: Vec<String>,
}

fn scan_dir(dir: &Path, depth_left: usize, at_root: bool, ctx: &mut Walk<'_>) {
    let (budget, found) = (&mut ctx.budget, &mut *ctx.found);
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
        if name.starts_with('.') && name != "."
            || name.starts_with('$') // $Recycle.Bin, $WinREAgent, ...
            || SKIP_DIRS.iter().any(|s| s == &name)
            || (at_root && ROOT_SKIP_DIRS.iter().any(|s| s.eq_ignore_ascii_case(&name)))
        {
            // Skip dotfolders and known-heavy generated dirs (but a `.wipe` at this
            // level was already handled above).
            continue;
        }
        let path = entry.path();
        if !ctx.skip.is_empty() && ctx.skip.contains(&norm(&path)) {
            continue; // walked as a root of its own
        }
        scan_dir(&path, depth_left - 1, false, ctx);
        if ctx.budget == 0 {
            return;
        }
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

        // Drive-root scans: OS trees right below a root are skipped (but a project
        // folder of the same name deeper down is not), `$`-folders are skipped,
        // and a root nested in another root is walked only once.
        let drive = tmp.path().join("drive");
        for (rel, name) in [
            ("Windows/proj-w", "W"),
            ("$Recycle.Bin/proj-r", "R"),
            ("work/Windows/proj-deep", "Deep"),
            ("home/proj-h", "H"),
        ] {
            let p = drive.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            Store::init(&p, name, chrono::Utc::now()).unwrap();
        }
        let home = drive.join("home");
        let found = scan(&[home.clone(), drive.clone()], 8);
        let names: Vec<String> = list().into_iter().map(|p| p.name).collect();
        assert!(
            names.contains(&"Deep".to_string()),
            "deep `Windows` folder still scanned"
        );
        assert!(names.contains(&"H".to_string()));
        assert!(
            !names.contains(&"W".to_string()),
            "root-level Windows skipped"
        );
        assert!(!names.contains(&"R".to_string()), "$-folders skipped");
        assert_eq!(found.len(), 2, "H found once (as its own root), plus Deep");

        std::env::remove_var("WIPE_CONFIG_DIR");
    }

    #[cfg(windows)]
    #[test]
    fn default_roots_include_the_system_drive() {
        let roots = default_scan_roots();
        let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        assert!(
            roots.iter().any(|r| r
                .display()
                .to_string()
                .eq_ignore_ascii_case(&format!("{sys}\\"))),
            "{roots:?}"
        );
    }
}
