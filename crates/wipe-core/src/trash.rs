//! Soft-delete "trash": deleting a ticket parks it in the gitignored cache with a
//! deletion timestamp (and its original list + position) so it can be restored,
//! and it is permanently purged once it is older than the user's retention window.
//!
//! The trash lives under `.wipe/.cache/trash/` - per-board but never committed, so
//! deletions don't pollute git history and each user keeps their own trash.

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::Ticket;
use crate::Store;

/// A ticket in the trash, with the context needed to restore it exactly where it
/// was and to report how long it has been deleted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrashEntry {
    /// The deleted ticket, verbatim.
    pub ticket: Ticket,
    /// When it was deleted.
    pub deleted_at: DateTime<Utc>,
    /// The list it was on when deleted (restored here, or the first list if that
    /// list no longer exists).
    pub list: String,
    /// Its 0-based position in that list at deletion time.
    pub index: usize,
}

fn entry_path(store: &Store, ticket_id: &str) -> PathBuf {
    store.trash_dir().join(format!("{ticket_id}.json"))
}

pub(crate) fn write_entry(store: &Store, entry: &TrashEntry) -> Result<()> {
    let dir = store.trash_dir();
    std::fs::create_dir_all(&dir).map_err(|e| Error::msg(e.to_string()))?;
    let path = entry_path(store, &entry.ticket.id);
    let mut json = serde_json::to_string_pretty(entry).map_err(|e| Error::msg(e.to_string()))?;
    json.push('\n');
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| Error::msg(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| Error::msg(e.to_string()))?;
    Ok(())
}

/// Every trash entry on disk (unsorted, expired ones included).
pub(crate) fn read_entries(store: &Store) -> Result<Vec<TrashEntry>> {
    read_all(store)
}

/// Remove the trash file stored under `ticket_id`, if any.
pub(crate) fn remove_entry_file(store: &Store, ticket_id: &str) -> Result<()> {
    match std::fs::remove_file(entry_path(store, ticket_id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::msg(e.to_string())),
        _ => Ok(()),
    }
}

/// The id of the trashed ticket a typed reference names (`T-01c`, `t1c`, a
/// pre-translation `T-28`), matched against the entries themselves - a typed
/// value never becomes a path, so `../board` can't reach outside the trash.
fn find_entry_id(store: &Store, raw: &str) -> Result<Option<String>> {
    let Some(digits) = crate::id::ticket_ref_digits(raw) else {
        return Ok(None);
    };
    let hex = store.load_board()?.ids == crate::model::IdFormat::Hex;
    let value = |radix| u64::from_str_radix(&digits, radix).ok();
    let entries = read_all(store)?;
    let same = |id: &str, radix: u32| {
        id.strip_prefix("T-")
            .and_then(|n| u64::from_str_radix(n, radix).ok())
            .is_some_and(|n| Some(n) == value(radix))
    };
    let hit = entries
        .iter()
        .find(|e| e.ticket.id == raw.trim())
        .or_else(|| {
            entries
                .iter()
                .find(|e| same(&e.ticket.id, if hex { 16 } else { 10 }))
        })
        .or_else(|| {
            entries
                .iter()
                .find(|e| e.ticket.legacy_id.as_deref().is_some_and(|l| same(l, 10)))
        });
    Ok(hit.map(|e| e.ticket.id.clone()))
}

/// Read every trash entry currently on disk (unsorted, not yet purged).
fn read_all(store: &Store) -> Result<Vec<TrashEntry>> {
    let dir = store.trash_dir();
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir).map_err(|e| Error::msg(e.to_string()))? {
        let entry = entry.map_err(|e| Error::msg(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(te) = serde_json::from_slice::<TrashEntry>(&bytes) {
                out.push(te);
            }
        }
    }
    Ok(out)
}

/// Soft-delete a ticket: move it (and its board card) into the trash. Returns the
/// created [`TrashEntry`]. When `retention_days` is `0` the ticket is deleted
/// outright (no trash kept).
pub fn trash_ticket(
    store: &Store,
    ticket_id: &str,
    retention_days: u64,
    now: DateTime<Utc>,
) -> Result<()> {
    let ticket_id = &store.resolve_ticket_id(ticket_id)?;
    let ticket = store.load_ticket(ticket_id)?; // errors if missing
    let mut board = store.load_board()?;

    // Capture the ticket's list + position before removing the card.
    let mut list = String::new();
    let mut index = 0usize;
    for l in &board.lists {
        if let Some(pos) = l.cards.iter().position(|c| c == ticket_id) {
            list = l.id.clone();
            index = pos;
            break;
        }
    }

    for l in &mut board.lists {
        l.cards.retain(|c| c != ticket_id);
    }

    // Park it in the trash (unless retention is disabled), then drop the file.
    if retention_days > 0 {
        write_entry(
            store,
            &TrashEntry {
                ticket,
                deleted_at: now,
                list,
                index,
            },
        )?;
    }
    store.delete_ticket(ticket_id)?;
    store.save_board(&board)?;
    Ok(())
}

/// List the trash, newest-deletion first, after purging anything older than
/// `retention_days` (so callers never see un-restorable entries).
pub fn list_trash(
    store: &Store,
    retention_days: u64,
    now: DateTime<Utc>,
) -> Result<Vec<TrashEntry>> {
    purge_expired(store, retention_days, now)?;
    let mut entries = read_all(store)?;
    entries.sort_by_key(|e| std::cmp::Reverse(e.deleted_at));
    Ok(entries)
}

/// Restore a trashed ticket back onto the board (into its original list, or the
/// first list if that list is gone), removing it from the trash. Errors if the
/// ticket is not in the trash.
pub fn restore_ticket(store: &Store, ticket_id: &str, now: DateTime<Utc>) -> Result<Ticket> {
    let ticket_id = &find_entry_id(store, ticket_id)?.unwrap_or_else(|| ticket_id.to_string());
    if crate::id::ticket_ref_digits(ticket_id).is_none() {
        return Err(Error::msg(format!("`{ticket_id}` is not in the trash")));
    }
    let path = entry_path(store, ticket_id);
    let bytes = std::fs::read(&path).map_err(|_| {
        Error::msg(format!(
            "`{ticket_id}` is not in the trash (already restored or purged)"
        ))
    })?;
    let entry: TrashEntry =
        serde_json::from_slice(&bytes).map_err(|e| Error::msg(e.to_string()))?;

    let mut board = store.load_board()?;
    // Prefer the original list; fall back to the first list if it's gone.
    let target = if board.list(&entry.list).is_some() {
        entry.list.clone()
    } else {
        board
            .lists
            .first()
            .map(|l| l.id.clone())
            .ok_or_else(|| Error::msg("board has no lists to restore into"))?
    };
    let dest = board.list_mut(&target).expect("checked above");
    // Guard against a stale index if the list shrank since deletion.
    let pos = entry.index.min(dest.cards.len());
    dest.cards.insert(pos, ticket_id.to_string());

    let mut ticket = entry.ticket;
    ticket.updated = now;
    store.save_ticket(&ticket)?;
    store.save_board(&board)?;
    let _ = std::fs::remove_file(&path);
    Ok(ticket)
}

/// Permanently delete a single trash entry (no restore afterwards). Returns
/// whether an entry was present.
pub fn purge_ticket(store: &Store, ticket_id: &str) -> Result<bool> {
    let Some(ticket_id) = find_entry_id(store, ticket_id)? else {
        return Ok(false);
    };
    let path = entry_path(store, &ticket_id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| Error::msg(e.to_string()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Empty the trash entirely. Returns how many entries were purged.
pub fn empty(store: &Store) -> Result<usize> {
    let ids: Vec<String> = read_all(store)?.into_iter().map(|e| e.ticket.id).collect();
    let mut n = 0;
    for id in ids {
        if purge_ticket(store, &id)? {
            n += 1;
        }
    }
    Ok(n)
}

/// Permanently remove entries older than `retention_days`. Returns how many were
/// purged. A `retention_days` of 0 purges everything.
pub fn purge_expired(store: &Store, retention_days: u64, now: DateTime<Utc>) -> Result<usize> {
    let cutoff = now - Duration::days(retention_days as i64);
    let mut n = 0;
    for entry in read_all(store)? {
        if entry.deleted_at <= cutoff && purge_ticket(store, &entry.ticket.id)? {
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{self, NewTicket};

    fn project() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Trash Test", Utc::now()).unwrap();
        (dir, store)
    }

    fn make(store: &Store, title: &str) -> String {
        let spec = NewTicket {
            title: title.into(),
            list: Some("todo".into()),
            ..Default::default()
        };
        ops::create_ticket(store, spec, "tester", Utc::now())
            .unwrap()
            .id
    }

    #[test]
    fn trash_and_restore_keep_list_position_and_never_touch_paths() {
        let (_d, s) = project();
        let (a, b, c) = (make(&s, "A"), make(&s, "B"), make(&s, "C"));
        let cards = |s: &Store| s.load_board().unwrap().list("todo").unwrap().cards.clone();

        trash_ticket(&s, &b, 7, Utc::now()).unwrap();
        assert!(s.load_ticket(&b).is_err());
        assert_eq!(cards(&s), [a.clone(), c.clone()]);
        assert_eq!(list_trash(&s, 7, Utc::now()).unwrap().len(), 1);

        // Typed refs are matched against the entries, never turned into paths:
        // `purge ../../board` once deleted .wipe/board.json (0.4.1).
        let board_json = s.wipe_dir().join("board.json");
        for evil in ["../board", "../../board", "..\\board"] {
            assert!(!purge_ticket(&s, evil).unwrap(), "{evil}");
            assert!(restore_ticket(&s, evil, Utc::now()).is_err(), "{evil}");
        }
        assert!(board_json.exists());

        // Restored to its old index, by any spelling of its id.
        restore_ticket(&s, &b.to_lowercase().replace('-', ""), Utc::now()).unwrap();
        assert_eq!(cards(&s), [a, b, c]);
        assert!(list_trash(&s, 7, Utc::now()).unwrap().is_empty());
    }

    #[test]
    fn retention_expires_entries_and_zero_deletes_outright() {
        let (_d, s) = project();
        let old = make(&s, "old");
        trash_ticket(&s, &old, 7, Utc::now() - Duration::days(10)).unwrap();
        assert!(
            list_trash(&s, 7, Utc::now()).unwrap().is_empty(),
            "expired entries purge"
        );
        assert!(restore_ticket(&s, &old, Utc::now()).is_err());

        let gone = make(&s, "gone");
        trash_ticket(&s, &gone, 0, Utc::now()).unwrap();
        assert!(list_trash(&s, 7, Utc::now()).unwrap().is_empty());
        assert!(s.load_ticket(&gone).is_err());
    }
}
