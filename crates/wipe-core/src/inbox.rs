//! Subscriptions and a non-blocking inbox.
//!
//! The inbox answers "what changed that I care about, and since when?" without
//! blocking. It scans ticket comments and activity plus forum posts, keeps only
//! events by *other* actors on objects the identity is assigned to, authored, or
//! subscribed to, and returns them newest-first. `--unread` advances a per-user
//! read cursor kept under the gitignored cache, so it never dirties the repo.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::{forum, Store};

/// One inbox item: something another actor did on an object you care about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxEvent {
    /// When it happened.
    pub ts: DateTime<Utc>,
    /// Event kind: `comment`, `activity`, or `forum`.
    pub kind: String,
    /// The object it happened on: a ticket `T-<n>` or a forum post id.
    pub object: String,
    /// A short title/context for the object.
    pub title: String,
    /// Who did it.
    pub actor: String,
    /// Human-readable detail (a comment/post snippet, or the activity detail).
    pub detail: String,
    /// Why it reached your inbox: `assigned`, `authored`, or `subscribed`.
    pub reason: String,
}

#[derive(Serialize, Deserialize, Default)]
struct CursorFile {
    cursor: Option<DateTime<Utc>>,
}

/// Add `reference` to `identity`'s subscriptions (idempotent). Returns whether it
/// was newly added.
pub fn subscribe(store: &Store, identity: &str, reference: &str) -> Result<bool> {
    let mut subs = store.load_subscriptions()?;
    let list = subs.subs.entry(identity.to_string()).or_default();
    if list.iter().any(|r| r == reference) {
        return Ok(false);
    }
    list.push(reference.to_string());
    list.sort();
    list.dedup();
    store.save_subscriptions(&subs)?;
    Ok(true)
}

/// Remove `reference` from `identity`'s subscriptions. Returns whether it was
/// present.
pub fn unsubscribe(store: &Store, identity: &str, reference: &str) -> Result<bool> {
    let mut subs = store.load_subscriptions()?;
    let Some(list) = subs.subs.get_mut(identity) else {
        return Ok(false);
    };
    let before = list.len();
    list.retain(|r| r != reference);
    let removed = list.len() != before;
    if list.is_empty() {
        subs.subs.remove(identity);
    }
    if removed {
        store.save_subscriptions(&subs)?;
    }
    Ok(removed)
}

/// The refs `identity` subscribes to (sorted).
pub fn subscriptions_of(store: &Store, identity: &str) -> Result<Vec<String>> {
    Ok(store
        .load_subscriptions()?
        .subs
        .get(identity)
        .cloned()
        .unwrap_or_default())
}

fn snippet(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n {
        one
    } else {
        format!("{}…", one.chars().take(n).collect::<String>())
    }
}

/// Compute the inbox for `identity`: events strictly after `since` on objects it
/// is assigned to, authored, or subscribed to - excluding the identity's own
/// actions. Sorted newest-first.
pub fn inbox(store: &Store, identity: &str, since: DateTime<Utc>) -> Result<Vec<InboxEvent>> {
    let subs = subscriptions_of(store, identity)?;
    let sub_set: HashSet<String> = subs.into_iter().collect();
    let all = sub_set.contains("board:*");
    let all_forum = all || sub_set.contains("forum:*");

    // ticket id -> its list id, for `list:<id>` subscriptions.
    let board = store.load_board()?;
    let mut ticket_list: BTreeMap<String, String> = BTreeMap::new();
    for l in &board.lists {
        for c in &l.cards {
            ticket_list.insert(c.clone(), l.id.clone());
        }
    }

    let mut events: Vec<InboxEvent> = Vec::new();

    for id in store.ticket_ids()? {
        let Ok(t) = store.load_ticket(&id) else {
            continue;
        };
        let assigned = t.assignees.iter().any(|a| a == identity);
        let authored = t
            .activity
            .iter()
            .any(|a| a.kind == "created" && a.actor == identity);
        let list_ref = ticket_list.get(&id).map(|l| format!("list:{l}"));
        let subbed =
            all || sub_set.contains(&id) || list_ref.map(|r| sub_set.contains(&r)).unwrap_or(false);
        if !(assigned || authored || subbed) {
            continue;
        }
        let reason = if assigned {
            "assigned"
        } else if authored {
            "authored"
        } else {
            "subscribed"
        };

        for c in &t.comments {
            if c.created > since && c.author != identity {
                events.push(InboxEvent {
                    ts: c.created,
                    kind: "comment".into(),
                    object: id.clone(),
                    title: t.title.clone(),
                    actor: c.author.clone(),
                    detail: snippet(&c.body, 120),
                    reason: reason.into(),
                });
            }
        }
        for a in &t.activity {
            if a.ts > since && a.actor != identity {
                let detail = format!("{} {}", a.kind, a.detail);
                events.push(InboxEvent {
                    ts: a.ts,
                    kind: "activity".into(),
                    object: id.clone(),
                    title: t.title.clone(),
                    actor: a.actor.clone(),
                    detail: detail.trim().to_string(),
                    reason: reason.into(),
                });
            }
        }
    }

    // Forum posts on subscribed threads (or all, via forum:* / board:*).
    for p in forum::index(store)? {
        let relevant = all_forum || sub_set.contains(&format!("forum:{}", p.thread_id));
        if relevant && p.created > since && p.author != identity {
            events.push(InboxEvent {
                ts: p.created,
                kind: "forum".into(),
                object: p.id.clone(),
                title: p.thread_title.clone(),
                actor: p.author.clone(),
                detail: snippet(&p.body, 120),
                reason: "subscribed".into(),
            });
        }
    }

    events.sort_by(|a, b| b.ts.cmp(&a.ts));
    Ok(events)
}

/// Read `identity`'s stored inbox read-cursor, if any.
pub fn read_cursor(store: &Store, identity: &str) -> Option<DateTime<Utc>> {
    let path = store.inbox_cursor_path(identity);
    std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<CursorFile>(&b).ok())
        .and_then(|c| c.cursor)
}

/// Advance `identity`'s inbox read-cursor to `ts` (gitignored, per-user state).
pub fn write_cursor(store: &Store, identity: &str, ts: DateTime<Utc>) -> Result<()> {
    let path = store.inbox_cursor_path(identity);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::msg(e.to_string()))?;
    }
    let json = serde_json::to_string(&CursorFile { cursor: Some(ts) }).unwrap_or_default();
    std::fs::write(&path, json).map_err(|e| Error::msg(e.to_string()))?;
    Ok(())
}
