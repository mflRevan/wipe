//! Deterministic, atomic persistence of a `.wipe` board and project discovery.
//!
//! [`Store`] is the *only* sanctioned way to read and write a `.wipe` directory.
//! All writes are:
//!
//! * **Deterministic** - `serde_json::to_string_pretty` plus a trailing newline,
//!   with a model whose field/collection order is stable, so re-serializing
//!   unchanged data yields byte-identical output and git diffs stay minimal.
//! * **Atomic** - written to a temporary file in the same directory and then
//!   renamed over the target, so a crash never leaves a half-written file.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{Board, Definitions, Identity, Settings, Thread, Ticket};

/// Name of the per-project board directory.
pub const WIPE_DIR: &str = ".wipe";

/// A handle to a `.wipe` board rooted at a project directory.
#[derive(Debug, Clone)]
pub struct Store {
    /// The project root (the directory that contains `.wipe`).
    root: PathBuf,
}

impl Store {
    /// The project root directory (the parent of `.wipe`).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to the `.wipe` directory.
    pub fn wipe_dir(&self) -> PathBuf {
        self.root.join(WIPE_DIR)
    }

    fn board_path(&self) -> PathBuf {
        self.wipe_dir().join("board.json")
    }

    fn definitions_path(&self) -> PathBuf {
        self.wipe_dir().join("definitions.json")
    }

    fn settings_path(&self) -> PathBuf {
        self.wipe_dir().join("settings.json")
    }

    fn tickets_dir(&self) -> PathBuf {
        self.wipe_dir().join("tickets")
    }

    /// Path to the media directory (version-controlled attachments).
    pub fn media_dir(&self) -> PathBuf {
        self.wipe_dir().join("media")
    }

    /// Path to the (gitignored) cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        self.wipe_dir().join(".cache")
    }

    fn ticket_path(&self, id: &str) -> PathBuf {
        self.tickets_dir().join(format!("{id}.json"))
    }

    /// Open an existing board rooted exactly at `root` (which must contain `.wipe`).
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if root.join(WIPE_DIR).is_dir() {
            Ok(Store {
                root: root.to_path_buf(),
            })
        } else {
            Err(Error::not_initialized(root))
        }
    }

    /// Discover the board by walking up from `start` until a `.wipe` directory is
    /// found, mirroring how git locates its repository root.
    pub fn discover(start: impl AsRef<Path>) -> Result<Self> {
        let start = start.as_ref();
        let abs = fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
        let mut cur: Option<PathBuf> = Some(abs);
        while let Some(dir) = cur {
            if dir.join(WIPE_DIR).is_dir() {
                return Ok(Store { root: dir });
            }
            cur = dir.parent().map(Path::to_path_buf);
        }
        Err(Error::not_initialized(start))
    }

    /// Initialize a brand-new board under `root` with the default (standard)
    /// starter content. Fails with [`Error::AlreadyInitialized`] if a `.wipe`
    /// directory already exists.
    pub fn init(
        root: impl AsRef<Path>,
        name: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Self> {
        Self::init_with(root, name, now, crate::model::Starter::Standard)
    }

    /// Initialize a brand-new board, choosing how much starter content to seed:
    /// standard lists+labels, lists only, or a blank board.
    pub fn init_with(
        root: impl AsRef<Path>,
        name: &str,
        now: chrono::DateTime<chrono::Utc>,
        starter: crate::model::Starter,
    ) -> Result<Self> {
        use crate::model::Starter;

        let abs = fs::canonicalize(root.as_ref())?;
        let wipe = abs.join(WIPE_DIR);
        if wipe.exists() {
            return Err(Error::AlreadyInitialized(wipe.display().to_string()));
        }
        fs::create_dir_all(wipe.join("tickets"))?;
        fs::create_dir_all(wipe.join("media"))?;
        fs::create_dir_all(wipe.join("forum"))?;
        fs::create_dir_all(wipe.join(".cache"))?;
        // Keep the local cache out of version control.
        write_bytes_atomic(&wipe.join(".gitignore"), b"/.cache/\n")?;
        // Keep the media and forum directories in git even when empty.
        write_bytes_atomic(&wipe.join("media").join(".gitkeep"), b"")?;
        write_bytes_atomic(&wipe.join("forum").join(".gitkeep"), b"")?;

        let board = match starter {
            Starter::Standard | Starter::ListsOnly => Board::new(name, now),
            Starter::Empty => Board::empty(name, now),
        };
        // Priorities are a harmless shared vocabulary, kept for every starter;
        // labels are only seeded for the standard starter.
        let mut defs = Definitions::seed();
        if starter != Starter::Standard {
            defs.labels.clear();
        }

        let store = Store { root: abs };
        store.save_board(&board)?;
        store.save_definitions(&defs)?;
        store.save_settings(&Settings::default())?;
        Ok(store)
    }

    // --- board -------------------------------------------------------------

    /// Load `board.json`.
    pub fn load_board(&self) -> Result<Board> {
        read_json(&self.board_path())
    }

    /// Write `board.json` deterministically and atomically.
    pub fn save_board(&self, board: &Board) -> Result<()> {
        write_json_atomic(&self.board_path(), board)
    }

    // --- definitions -------------------------------------------------------

    /// Load `definitions.json`.
    pub fn load_definitions(&self) -> Result<Definitions> {
        read_json(&self.definitions_path())
    }

    /// Write `definitions.json`.
    pub fn save_definitions(&self, defs: &Definitions) -> Result<()> {
        write_json_atomic(&self.definitions_path(), defs)
    }

    // --- settings ----------------------------------------------------------

    /// Load `settings.json`.
    pub fn load_settings(&self) -> Result<Settings> {
        read_json(&self.settings_path())
    }

    /// Write `settings.json`.
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        write_json_atomic(&self.settings_path(), settings)
    }

    // --- identities --------------------------------------------------------

    fn identities_path(&self) -> PathBuf {
        self.wipe_dir().join("identities.json")
    }

    /// Load `identities.json` (empty if the file doesn't exist yet).
    pub fn load_identities(&self) -> Result<Vec<Identity>> {
        let path = self.identities_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        read_json(&path)
    }

    /// Write `identities.json`.
    pub fn save_identities(&self, identities: &[Identity]) -> Result<()> {
        write_json_atomic(&self.identities_path(), identities)
    }

    // --- subscriptions -----------------------------------------------------

    fn subscriptions_path(&self) -> PathBuf {
        self.wipe_dir().join("subscriptions.json")
    }

    /// Load `subscriptions.json` (empty if the file doesn't exist yet).
    pub fn load_subscriptions(&self) -> Result<crate::model::Subscriptions> {
        let path = self.subscriptions_path();
        if !path.exists() {
            return Ok(crate::model::Subscriptions::default());
        }
        read_json(&path)
    }

    /// Write `subscriptions.json`.
    pub fn save_subscriptions(&self, subs: &crate::model::Subscriptions) -> Result<()> {
        write_json_atomic(&self.subscriptions_path(), subs)
    }

    /// Path to `identity`'s (gitignored) inbox read-cursor file under `.cache/`.
    /// Per-user state, so it never dirties the repo or conflicts on merge.
    pub fn inbox_cursor_path(&self, identity: &str) -> PathBuf {
        let slug: String = identity
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        self.cache_dir().join("inbox").join(format!("{slug}.json"))
    }

    // --- tickets -----------------------------------------------------------

    /// Load a single ticket by ID.
    pub fn load_ticket(&self, id: &str) -> Result<Ticket> {
        if !valid_ticket_id(id) {
            return Err(Error::TicketNotFound(id.to_string()));
        }
        let path = self.ticket_path(id);
        if !path.exists() {
            return Err(Error::TicketNotFound(id.to_string()));
        }
        read_json(&path)
    }

    /// Write a ticket file.
    pub fn save_ticket(&self, ticket: &Ticket) -> Result<()> {
        if !valid_ticket_id(&ticket.id) {
            return Err(Error::msg(format!("invalid ticket id `{}`", ticket.id)));
        }
        write_json_atomic(&self.ticket_path(&ticket.id), ticket)
    }

    /// Delete a ticket file. Errors if it does not exist.
    pub fn delete_ticket(&self, id: &str) -> Result<()> {
        if !valid_ticket_id(id) {
            return Err(Error::TicketNotFound(id.to_string()));
        }
        let path = self.ticket_path(id);
        if !path.exists() {
            return Err(Error::TicketNotFound(id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    /// Return all ticket IDs currently on disk, sorted numerically by counter.
    pub fn ticket_ids(&self) -> Result<Vec<String>> {
        let dir = self.tickets_dir();
        let mut ids: Vec<String> = Vec::new();
        if !dir.exists() {
            return Ok(ids);
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(stem.to_string());
                }
            }
        }
        ids.sort_by_key(|id| ticket_counter(id).unwrap_or(u64::MAX));
        Ok(ids)
    }

    /// Load every ticket on disk, ordered by ID counter.
    pub fn load_all_tickets(&self) -> Result<Vec<Ticket>> {
        self.ticket_ids()?
            .iter()
            .map(|id| self.load_ticket(id))
            .collect()
    }

    // --- forum -------------------------------------------------------------

    /// Path to the forum directory (`.wipe/forum`).
    pub fn forum_dir(&self) -> PathBuf {
        self.wipe_dir().join("forum")
    }

    fn thread_path(&self, thread_id: &str) -> PathBuf {
        self.forum_dir().join(format!("{thread_id}.json"))
    }

    /// Whether a thread file already exists on disk for `thread_id`.
    pub fn thread_exists(&self, thread_id: &str) -> bool {
        valid_thread_id(thread_id) && self.thread_path(thread_id).exists()
    }

    /// Load a forum thread by its thread ID (e.g. `F-1`).
    pub fn load_thread(&self, thread_id: &str) -> Result<Thread> {
        if !valid_thread_id(thread_id) {
            return Err(Error::ThreadNotFound(thread_id.to_string()));
        }
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Err(Error::ThreadNotFound(thread_id.to_string()));
        }
        read_json(&path)
    }

    /// Write a forum thread file.
    pub fn save_thread(&self, thread: &Thread) -> Result<()> {
        if !valid_thread_id(&thread.id) {
            return Err(Error::msg(format!("invalid thread id `{}`", thread.id)));
        }
        write_json_atomic(&self.thread_path(&thread.id), thread)
    }

    /// Delete a forum thread file. Errors if it does not exist.
    pub fn delete_thread(&self, thread_id: &str) -> Result<()> {
        if !valid_thread_id(thread_id) {
            return Err(Error::ThreadNotFound(thread_id.to_string()));
        }
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Err(Error::ThreadNotFound(thread_id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    /// All forum thread IDs on disk, sorted numerically by counter.
    pub fn thread_ids(&self) -> Result<Vec<String>> {
        let dir = self.forum_dir();
        let mut ids: Vec<String> = Vec::new();
        if !dir.exists() {
            return Ok(ids);
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(stem.to_string());
                }
            }
        }
        ids.sort_by_key(|id| thread_counter(id).unwrap_or(u64::MAX));
        Ok(ids)
    }

    /// Load every forum thread on disk, ordered by ID counter.
    pub fn load_all_threads(&self) -> Result<Vec<Thread>> {
        self.thread_ids()?
            .iter()
            .map(|id| self.load_thread(id))
            .collect()
    }
}

/// A ticket ID must be `T-` followed by digits only. Rejecting anything else
/// (path separators, `..`, dots) guarantees a caller-supplied ID can never be
/// turned into a path that escapes the tickets directory - e.g. a crafted
/// `../board` must never let `delete_ticket` remove `.wipe/board.json`.
fn valid_ticket_id(id: &str) -> bool {
    matches!(id.strip_prefix("T-"), Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// A thread ID must be `F-` followed by digits only. Rejecting anything else
/// (path separators, `..`, dots) guarantees a caller-supplied ID can never be
/// turned into a path that escapes the forum directory.
fn valid_thread_id(id: &str) -> bool {
    matches!(id.strip_prefix("F-"), Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Parse the numeric counter out of an `F-<n>` thread ID (ignoring any `.x` tail).
fn thread_counter(id: &str) -> Option<u64> {
    id.strip_prefix("F-")
        .map(|rest| rest.split('.').next().unwrap_or(rest))
        .and_then(|n| n.parse().ok())
}

/// Parse the numeric counter out of a `T-<n>` ticket ID.
fn ticket_counter(id: &str) -> Option<u64> {
    id.strip_prefix("T-").and_then(|n| n.parse().ok())
}

// --- low-level IO ----------------------------------------------------------

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| Error::msg(format!("path `{}` has no parent", path.display())))?;
    fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| Error::Io(e.error))?;
    Ok(())
}

fn write_json_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let mut s = serde_json::to_string_pretty(value)?;
    s.push('\n');
    write_bytes_atomic(path, s.as_bytes())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Ticket;
    use chrono::{TimeZone, Utc};

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0).unwrap()
    }

    fn temp_project() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Test Board", now()).unwrap();
        (dir, store)
    }

    #[test]
    fn init_creates_layout() {
        let (_dir, store) = temp_project();
        assert!(store.wipe_dir().join("board.json").is_file());
        assert!(store.wipe_dir().join("definitions.json").is_file());
        assert!(store.wipe_dir().join("settings.json").is_file());
        assert!(store.wipe_dir().join("tickets").is_dir());
        assert!(store.wipe_dir().join("media").is_dir());
        assert!(store.wipe_dir().join(".gitignore").is_file());
    }

    #[test]
    fn init_twice_fails() {
        let (dir, _store) = temp_project();
        let err = Store::init(dir.path(), "Again", now()).unwrap_err();
        assert!(matches!(err, Error::AlreadyInitialized(_)));
    }

    #[test]
    fn discover_walks_up() {
        let (dir, _store) = temp_project();
        let nested = dir.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let found = Store::discover(&nested).unwrap();
        assert_eq!(
            fs::canonicalize(found.root()).unwrap(),
            fs::canonicalize(dir.path()).unwrap()
        );
    }

    #[test]
    fn ticket_roundtrip_and_ordering() {
        let (_dir, store) = temp_project();
        for n in [1u64, 2, 10] {
            let t = Ticket::new(format!("T-{n}"), format!("Ticket {n}"), now());
            store.save_ticket(&t).unwrap();
        }
        // Numeric, not lexical, ordering: T-2 before T-10.
        assert_eq!(store.ticket_ids().unwrap(), vec!["T-1", "T-2", "T-10"]);
        let loaded = store.load_ticket("T-10").unwrap();
        assert_eq!(loaded.title, "Ticket 10");
    }

    #[test]
    fn missing_ticket_errors() {
        let (_dir, store) = temp_project();
        assert!(matches!(
            store.load_ticket("T-99"),
            Err(Error::TicketNotFound(_))
        ));
    }

    #[test]
    fn ticket_id_traversal_is_rejected() {
        let (_dir, store) = temp_project();
        // A crafted id must never resolve to a path outside tickets/. board.json
        // exists next to tickets/, so if traversal worked delete would remove it.
        let board_json = store.wipe_dir().join("board.json");
        assert!(board_json.exists());
        for evil in ["../board", "../../evil", "T-1/../../board", "..\\board"] {
            assert!(matches!(
                store.delete_ticket(evil),
                Err(Error::TicketNotFound(_))
            ));
            assert!(matches!(
                store.load_ticket(evil),
                Err(Error::TicketNotFound(_))
            ));
        }
        // The sibling file is untouched.
        assert!(board_json.exists());
    }

    #[test]
    fn serialization_is_deterministic_and_newline_terminated() {
        let (_dir, store) = temp_project();
        let raw = fs::read_to_string(store.wipe_dir().join("board.json")).unwrap();
        assert!(raw.ends_with('\n'));
        // Round-trip: load and re-save yields byte-identical output.
        let board = store.load_board().unwrap();
        store.save_board(&board).unwrap();
        let raw2 = fs::read_to_string(store.wipe_dir().join("board.json")).unwrap();
        assert_eq!(raw, raw2);
    }
}
