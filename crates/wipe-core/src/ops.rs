//! High-level, transactional board operations shared by every front-end
//! (CLI, daemon, desktop). Each function loads what it needs through [`Store`],
//! mutates the in-memory model, and writes it back deterministically. Keeping
//! these here means the mutation rules live in exactly one place.

use std::fs;

use chrono::{DateTime, Utc};

use crate::error::{Error, Result};
use crate::git;
use crate::id::{slug, ticket_id};
use crate::model::{Attachment, AttachmentSource, Board, Identity, IdentityKind, List, Ticket};
use crate::store::Store;

/// Specification for a new ticket. Only `title` is required.
#[derive(Debug, Default, Clone)]
pub struct NewTicket {
    /// Short title.
    pub title: String,
    /// Optional long-form body.
    pub body: Option<String>,
    /// Optional ticket type.
    pub kind: Option<String>,
    /// Optional priority.
    pub priority: Option<String>,
    /// Target list ID; defaults to the board's first list.
    pub list: Option<String>,
    /// Labels to apply.
    pub labels: Vec<String>,
    /// Tags to apply.
    pub tags: Vec<String>,
    /// Assignees.
    pub assignees: Vec<String>,
}

/// Create a ticket, allocate its ID, place it on a list, and persist both the
/// ticket file and the board. Returns the created ticket.
pub fn create_ticket(store: &Store, spec: NewTicket, now: DateTime<Utc>) -> Result<Ticket> {
    let mut board = store.load_board()?;

    let list_id = match spec.list {
        Some(l) => {
            if board.list(&l).is_none() {
                return Err(Error::ListNotFound(l));
            }
            l
        }
        None => board
            .lists
            .first()
            .map(|l| l.id.clone())
            .ok_or_else(|| Error::msg("board has no lists"))?,
    };

    let id = ticket_id(board.next_ticket);
    board.next_ticket += 1;

    let mut ticket = Ticket::new(id.clone(), spec.title, now);
    ticket.body = spec.body.unwrap_or_default();
    ticket.kind = spec.kind;
    ticket.priority = spec.priority;
    ticket.labels = spec.labels;
    ticket.tags = spec.tags;
    ticket.assignees = spec.assignees;

    board
        .list_mut(&list_id)
        .expect("checked above")
        .cards
        .push(id.clone());
    board.updated = now;

    store.save_ticket(&ticket)?;
    store.save_board(&board)?;
    Ok(ticket)
}

/// Move a ticket to `to_list` at an optional 0-based `position` (appended if
/// `None`). Removes it from whatever list currently holds it.
pub fn move_ticket(
    store: &Store,
    ticket_id: &str,
    to_list: &str,
    position: Option<usize>,
    now: DateTime<Utc>,
) -> Result<()> {
    // Ensure the ticket exists.
    let _ = store.load_ticket(ticket_id)?;
    let mut board = store.load_board()?;
    if board.list(to_list).is_none() {
        return Err(Error::ListNotFound(to_list.to_string()));
    }

    // Remove from current list (if present).
    for list in &mut board.lists {
        list.cards.retain(|c| c != ticket_id);
    }

    let dest = board.list_mut(to_list).expect("checked above");
    let pos = position.unwrap_or(dest.cards.len()).min(dest.cards.len());
    dest.cards.insert(pos, ticket_id.to_string());
    board.updated = now;

    // Touch the ticket so its own `updated` reflects the move.
    let mut ticket = store.load_ticket(ticket_id)?;
    ticket.updated = now;
    store.save_ticket(&ticket)?;
    store.save_board(&board)?;
    Ok(())
}

/// Delete a ticket file and remove its card from the board.
pub fn delete_ticket(store: &Store, ticket_id: &str, now: DateTime<Utc>) -> Result<()> {
    store.delete_ticket(ticket_id)?; // errors if missing
    let mut board = store.load_board()?;
    for list in &mut board.lists {
        list.cards.retain(|c| c != ticket_id);
    }
    board.updated = now;
    store.save_board(&board)?;
    Ok(())
}

/// Append a comment to a ticket. Returns the new comment ID.
pub fn add_comment(
    store: &Store,
    ticket_id: &str,
    author: &str,
    body: &str,
    now: DateTime<Utc>,
) -> Result<String> {
    let mut ticket = store.load_ticket(ticket_id)?;
    let id = ticket.add_comment(author, body, now);
    store.save_ticket(&ticket)?;
    Ok(id)
}

/// Add a new list to the end of the board. Returns the new list's ID.
pub fn add_list(store: &Store, name: &str, now: DateTime<Utc>) -> Result<String> {
    let mut board = store.load_board()?;
    let mut id = slug(name);
    // Ensure the slug is unique.
    if board.list(&id).is_some() {
        let mut n = 2;
        loop {
            let candidate = format!("{id}-{n}");
            if board.list(&candidate).is_none() {
                id = candidate;
                break;
            }
            n += 1;
        }
    }
    let mut list = List::new(name);
    list.id = id.clone();
    board.lists.push(list);
    board.updated = now;
    store.save_board(&board)?;
    Ok(id)
}

/// Remove a list. Fails if the list still holds cards, unless `force` is set (in
/// which case the contained tickets are also deleted).
pub fn remove_list(store: &Store, list_id: &str, force: bool, now: DateTime<Utc>) -> Result<()> {
    let mut board = store.load_board()?;
    let idx = board
        .lists
        .iter()
        .position(|l| l.id == list_id)
        .ok_or_else(|| Error::ListNotFound(list_id.to_string()))?;

    let cards = board.lists[idx].cards.clone();
    if !cards.is_empty() && !force {
        return Err(Error::msg(format!(
            "list `{list_id}` still holds {} ticket(s); pass --force to delete them too",
            cards.len()
        )));
    }
    for id in cards {
        // Ignore missing files; the goal state is "gone".
        let _ = store.delete_ticket(&id);
    }
    board.lists.remove(idx);
    board.updated = now;
    store.save_board(&board)?;
    Ok(())
}

/// Reorder a list to a new 0-based index.
pub fn move_list(store: &Store, list_id: &str, to_index: usize, now: DateTime<Utc>) -> Result<()> {
    let mut board = store.load_board()?;
    let from = board
        .lists
        .iter()
        .position(|l| l.id == list_id)
        .ok_or_else(|| Error::ListNotFound(list_id.to_string()))?;
    let list = board.lists.remove(from);
    let to = to_index.min(board.lists.len());
    board.lists.insert(to, list);
    board.updated = now;
    store.save_board(&board)?;
    Ok(())
}

/// Rename a list's display name (its ID stays stable).
pub fn rename_list(store: &Store, list_id: &str, name: &str, now: DateTime<Utc>) -> Result<()> {
    let mut board = store.load_board()?;
    let list = board
        .list_mut(list_id)
        .ok_or_else(|| Error::ListNotFound(list_id.to_string()))?;
    list.name = name.to_string();
    board.updated = now;
    store.save_board(&board)?;
    Ok(())
}

/// One list's ID paired with the tickets currently on it, in card order.
pub type ListView = (String, Vec<Ticket>);

/// Load the whole board as an ordered sequence of `(list_id, tickets)`.
pub fn board_view(store: &Store) -> Result<(Board, Vec<ListView>)> {
    let board = store.load_board()?;
    let mut out = Vec::with_capacity(board.lists.len());
    for list in &board.lists {
        let mut tickets = Vec::with_capacity(list.cards.len());
        for id in &list.cards {
            // Skip dangling references rather than failing the whole view.
            if let Ok(t) = store.load_ticket(id) {
                tickets.push(t);
            }
        }
        out.push((list.id.clone(), tickets));
    }
    Ok((board, out))
}

/// A partial update to a ticket. `None` fields are left unchanged. For `kind` and
/// `priority`, an inner `Some(None)` clears the value.
#[derive(Debug, Default, Clone)]
pub struct TicketPatch {
    /// New title.
    pub title: Option<String>,
    /// New body.
    pub body: Option<String>,
    /// Set/clear the ticket type.
    pub kind: Option<Option<String>>,
    /// Set/clear the priority.
    pub priority: Option<Option<String>>,
    /// Replace the label set.
    pub labels: Option<Vec<String>>,
    /// Replace the tag set.
    pub tags: Option<Vec<String>>,
    /// Replace the assignee set.
    pub assignees: Option<Vec<String>>,
}

/// Apply a [`TicketPatch`] and persist. Returns the updated ticket.
pub fn update_ticket(
    store: &Store,
    id: &str,
    patch: TicketPatch,
    now: DateTime<Utc>,
) -> Result<Ticket> {
    let mut t = store.load_ticket(id)?;
    if let Some(v) = patch.title {
        t.title = v;
    }
    if let Some(v) = patch.body {
        t.body = v;
    }
    if let Some(v) = patch.kind {
        t.kind = v;
    }
    if let Some(v) = patch.priority {
        t.priority = v;
    }
    if let Some(v) = patch.labels {
        t.labels = v;
    }
    if let Some(v) = patch.tags {
        t.tags = v;
    }
    if let Some(v) = patch.assignees {
        t.assignees = v;
    }
    t.updated = now;
    store.save_ticket(&t)?;
    Ok(t)
}

/// List identities: the saved registry merged with human authors discovered from
/// git (so contributors show up without manual setup).
pub fn list_identities(store: &Store) -> Result<Vec<Identity>> {
    let mut registry = store.load_identities()?;
    if git::is_repo(store.root()) {
        if let Ok(authors) = git::authors(store.root()) {
            for (name, email) in authors {
                if !registry.iter().any(|i| i.id == email) {
                    registry.push(Identity {
                        id: email,
                        display_name: name,
                        kind: IdentityKind::Human,
                    });
                }
            }
        }
    }
    Ok(registry)
}

/// Create or update an identity's display name (and optionally its kind).
pub fn upsert_identity(
    store: &Store,
    id: &str,
    display_name: &str,
    kind: Option<IdentityKind>,
) -> Result<Identity> {
    let mut registry = store.load_identities()?;
    if let Some(existing) = registry.iter_mut().find(|i| i.id == id) {
        existing.display_name = display_name.to_string();
        if let Some(k) = kind {
            existing.kind = k;
        }
        let out = existing.clone();
        store.save_identities(&registry)?;
        Ok(out)
    } else {
        let ident = Identity {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind: kind.unwrap_or_default(),
        };
        registry.push(ident.clone());
        store.save_identities(&registry)?;
        Ok(ident)
    }
}

/// Attach a file to a ticket. If a file with identical content is already tracked
/// in the repo, it is referenced in place (no copy); otherwise the bytes are
/// stored under `.wipe/media/`. Returns the created [`Attachment`].
pub fn add_attachment(
    store: &Store,
    ticket_id: &str,
    name: &str,
    bytes: &[u8],
    mime: &str,
    now: DateTime<Utc>,
) -> Result<Attachment> {
    let mut ticket = store.load_ticket(ticket_id)?;
    let root = store.root();
    let hash = git::blob_hash(bytes);

    // Prefer referencing an already-tracked file with identical content.
    let existing = if git::is_repo(root) {
        git::tracked_blobs(root)
            .ok()
            .and_then(|blobs| blobs.into_iter().find(|(h, _)| *h == hash).map(|(_, p)| p))
    } else {
        None
    };

    let attachment = if let Some(path) = existing {
        Attachment {
            name: name.to_string(),
            path,
            source: AttachmentSource::Repo,
            size: bytes.len() as u64,
            mime: mime.to_string(),
        }
    } else {
        let file_name = format!("{}-{}", &hash[..8.min(hash.len())], sanitize(name));
        fs::create_dir_all(store.media_dir())?;
        fs::write(store.media_dir().join(&file_name), bytes)?;
        Attachment {
            name: name.to_string(),
            path: format!(".wipe/media/{file_name}"),
            source: AttachmentSource::Media,
            size: bytes.len() as u64,
            mime: mime.to_string(),
        }
    };

    if !ticket.attachments.iter().any(|a| a.path == attachment.path) {
        ticket.attachments.push(attachment.clone());
        ticket.updated = now;
        store.save_ticket(&ticket)?;
    }
    Ok(attachment)
}

/// Detach a file from a ticket by its `path`. The underlying file is left in place
/// (it may be shared or tracked in the repo).
pub fn remove_attachment(
    store: &Store,
    ticket_id: &str,
    path: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let mut ticket = store.load_ticket(ticket_id)?;
    ticket.attachments.retain(|a| a.path != path);
    ticket.updated = now;
    store.save_ticket(&ticket)?;
    Ok(())
}

/// Sanitize a file name to a safe, stable form for on-disk storage.
fn sanitize(name: &str) -> String {
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0).unwrap()
    }

    fn project() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Test", now()).unwrap();
        (dir, store)
    }

    #[test]
    fn create_places_on_first_list_and_allocates_ids() {
        let (_d, s) = project();
        let t1 = create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        let t2 = create_ticket(
            &s,
            NewTicket {
                title: "B".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        assert_eq!(t1.id, "T-1");
        assert_eq!(t2.id, "T-2");
        let board = s.load_board().unwrap();
        assert_eq!(board.lists[0].cards, vec!["T-1", "T-2"]);
        assert_eq!(board.next_ticket, 3);
    }

    #[test]
    fn move_relocates_card() {
        let (_d, s) = project();
        create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        move_ticket(&s, "T-1", "done", None, now()).unwrap();
        let board = s.load_board().unwrap();
        assert!(board.list("backlog").unwrap().cards.is_empty());
        assert_eq!(board.list("done").unwrap().cards, vec!["T-1"]);
    }

    #[test]
    fn move_to_unknown_list_errors() {
        let (_d, s) = project();
        create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        assert!(matches!(
            move_ticket(&s, "T-1", "nope", None, now()),
            Err(Error::ListNotFound(_))
        ));
    }

    #[test]
    fn delete_removes_file_and_card() {
        let (_d, s) = project();
        create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        delete_ticket(&s, "T-1", now()).unwrap();
        assert!(matches!(
            s.load_ticket("T-1"),
            Err(Error::TicketNotFound(_))
        ));
        let board = s.load_board().unwrap();
        assert!(board.lists.iter().all(|l| l.cards.is_empty()));
    }

    #[test]
    fn list_lifecycle() {
        let (_d, s) = project();
        let id = add_list(&s, "In Review", now()).unwrap();
        assert_eq!(id, "in-review");
        rename_list(&s, &id, "Review", now()).unwrap();
        move_list(&s, &id, 0, now()).unwrap();
        let board = s.load_board().unwrap();
        assert_eq!(board.lists[0].id, "in-review");
        assert_eq!(board.lists[0].name, "Review");
        remove_list(&s, &id, false, now()).unwrap();
        assert!(s.load_board().unwrap().list("in-review").is_none());
    }

    #[test]
    fn remove_nonempty_list_requires_force() {
        let (_d, s) = project();
        create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        assert!(remove_list(&s, "backlog", false, now()).is_err());
        remove_list(&s, "backlog", true, now()).unwrap();
        assert!(matches!(
            s.load_ticket("T-1"),
            Err(Error::TicketNotFound(_))
        ));
    }

    #[test]
    fn update_ticket_applies_patch() {
        let (_d, s) = project();
        create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        let patch = TicketPatch {
            title: Some("A2".into()),
            priority: Some(Some("high".into())),
            labels: Some(vec!["blocked".into()]),
            assignees: Some(vec!["ada@example.com".into()]),
            ..Default::default()
        };
        let t = update_ticket(&s, "T-1", patch, now()).unwrap();
        assert_eq!(t.title, "A2");
        assert_eq!(t.priority.as_deref(), Some("high"));
        assert_eq!(t.labels, vec!["blocked"]);
        let cleared = update_ticket(
            &s,
            "T-1",
            TicketPatch {
                priority: Some(None),
                ..Default::default()
            },
            now(),
        )
        .unwrap();
        assert_eq!(cleared.priority, None);
    }

    #[test]
    fn identity_upsert_and_list() {
        let (_d, s) = project();
        upsert_identity(&s, "claude", "Claude", Some(IdentityKind::Agent)).unwrap();
        let ids = list_identities(&s).unwrap();
        let agent = ids.iter().find(|i| i.id == "claude").unwrap();
        assert_eq!(agent.display_name, "Claude");
        assert_eq!(agent.kind, IdentityKind::Agent);
    }

    #[test]
    fn attachment_prefers_repo_reference_over_copy() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| {
            assert!(Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Tester"]);
        let s = Store::init(root, "Att", now()).unwrap();
        create_ticket(
            &s,
            NewTicket {
                title: "A".into(),
                ..Default::default()
            },
            now(),
        )
        .unwrap();

        std::fs::write(root.join("logo.png"), b"PNGDATA").unwrap();
        git(&["add", "logo.png"]);
        git(&["commit", "-q", "-m", "add logo"]);

        // Identical bytes -> reference the tracked repo file (no copy).
        let a = add_attachment(&s, "T-1", "logo.png", b"PNGDATA", "image/png", now()).unwrap();
        assert_eq!(a.source, AttachmentSource::Repo);
        assert_eq!(a.path, "logo.png");

        // Novel bytes -> copied into .wipe/media/.
        let b = add_attachment(&s, "T-1", "new.txt", b"hello world", "text/plain", now()).unwrap();
        assert_eq!(b.source, AttachmentSource::Media);
        assert!(b.path.starts_with(".wipe/media/"));
        assert!(root.join(&b.path).exists());
    }
}
