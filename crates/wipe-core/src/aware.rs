//! Awareness between commands: "what changed on the board since I last looked?"
//!
//! An agent never looks at the board on its own, so a ticket someone files for
//! it - or a comment on the ticket it is working on - goes unnoticed until it
//! happens to run `wipe status`. [`catch_up`] closes that gap without hooks: each
//! wipe command diffs the board against a snapshot kept per identity (the
//! [`Watcher`] state, under the gitignored cache), returns the changes that
//! concern that identity, and saves the new snapshot, so each change is reported
//! exactly once - whatever command happens to run next, in whatever shell.
//!
//! What concerns an identity: tickets anyone else created; on tickets assigned
//! to it (or subscribed to): moves, comments, edits, (un)assignment and deletion;
//! comments and posts that @mention it; and new posts in forum threads it
//! started, replied in, or subscribes to. Its own actions never count. The first
//! catch-up on a board only records the baseline.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::events::{Event, WatchState, Watcher};
use crate::{forum, inbox, Store};

/// One change worth telling an identity about.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The change.
    pub event: Event,
    /// Why it concerns the identity: `new` (a ticket someone created),
    /// `assigned` (its ticket), `subscribed`, `mention`, or `thread` (a forum
    /// thread it started or replied in).
    pub why: &'static str,
    /// For forum posts: the thread id (`F-3`).
    pub thread: Option<String>,
}

fn slug(identity: &str) -> String {
    identity
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn snapshot_path(store: &Store, identity: &str) -> PathBuf {
    store
        .cache_dir()
        .join("aware")
        .join(format!("{}.json", slug(identity)))
}

/// Whether `text` mentions `@identity` (not as a prefix of a longer name).
pub fn mentions(text: &str, identity: &str) -> bool {
    let needle = format!("@{identity}");
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        let after = &rest[i + needle.len()..];
        if !after
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return true;
        }
        rest = after;
    }
    false
}

/// Report what changed on the board for `identity` since its last catch-up and
/// advance its snapshot (oldest first). Concurrent catch-ups by the same
/// identity are serialized, so a change is never reported twice.
pub fn catch_up(store: &Store, identity: &str) -> Result<Vec<Update>> {
    let path = snapshot_path(store, identity);
    let dir = path.parent().expect("snapshot path has a parent");
    std::fs::create_dir_all(dir).map_err(|e| Error::msg(e.to_string()))?;
    let lock = File::create(path.with_extension("lock")).map_err(|e| Error::msg(e.to_string()))?;
    lock.lock().map_err(|e| Error::msg(e.to_string()))?;

    let saved = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<WatchState>(&b).ok());
    let (watcher, events) = match saved {
        Some(state) => {
            let mut w = Watcher::resume(store.clone(), state);
            let events = w.poll();
            (w, events)
        }
        // First look at this board: the baseline, nothing to report yet.
        None => (Watcher::new(store.clone())?, Vec::new()),
    };
    if watcher.is_dirty() {
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_vec(watcher.state()).map_err(|e| Error::msg(e.to_string()))?;
        std::fs::write(&tmp, json).map_err(|e| Error::msg(e.to_string()))?;
        std::fs::rename(&tmp, &path).map_err(|e| Error::msg(e.to_string()))?;
    }
    Ok(relevant(store, identity, events))
}

/// Keep the events that concern `identity`, with the reason.
fn relevant(store: &Store, identity: &str, events: Vec<Event>) -> Vec<Update> {
    let events: Vec<Event> = events.into_iter().filter(|e| e.by != identity).collect();
    if events.is_empty() {
        return Vec::new();
    }
    let subs: HashSet<String> = inbox::subscriptions_of(store, identity)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let everything = subs.contains("board:*");

    // Forum context, only when a post is among the changes: each post's thread,
    // and the threads this identity started or replied in.
    let mut post_thread: HashMap<String, String> = HashMap::new();
    let mut own_threads: HashSet<String> = HashSet::new();
    if events.iter().any(|e| e.event == "posted") {
        for p in forum::index(store).unwrap_or_default() {
            if p.author == identity {
                own_threads.insert(p.thread_id.clone());
            }
            post_thread.insert(p.id, p.thread_id);
        }
    }

    let mut out = Vec::new();
    for e in events {
        let mine = e.assignees.iter().any(|a| a == identity);
        let why = match e.event {
            "posted" => {
                let thread = post_thread.get(&e.id).cloned();
                let why = match &thread {
                    _ if mentions(&e.detail, identity) => Some("mention"),
                    Some(t) if own_threads.contains(t) => Some("thread"),
                    Some(t)
                        if everything
                            || subs.contains("forum:*")
                            || subs.contains(&format!("forum:{t}")) =>
                    {
                        Some("subscribed")
                    }
                    _ => None,
                };
                if let Some(why) = why {
                    out.push(Update {
                        event: e,
                        why,
                        thread,
                    });
                }
                continue;
            }
            "created" if mine => Some("assigned"),
            "created" => Some("new"),
            // Taken off this identity's ticket: still its business to hear.
            "assigned" if e.detail == format!("-{identity}") => Some("assigned"),
            _ if mine => Some("assigned"),
            "commented" if mentions(&e.detail, identity) => Some("mention"),
            _ if everything
                || subs.contains(&e.id)
                || e.list
                    .as_ref()
                    .or(e.from.as_ref())
                    .is_some_and(|l| subs.contains(&format!("list:{l}"))) =>
            {
                Some("subscribed")
            }
            _ => None,
        };
        if let Some(why) = why {
            out.push(Update {
                event: e,
                why,
                thread: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{self, NewTicket, TicketPatch};
    use chrono::Utc;

    fn board() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::init(d.path(), "A", Utc::now()).unwrap();
        (d, s)
    }

    fn new(s: &Store, title: &str, by: &str, assignees: &[&str]) -> String {
        ops::create_ticket(
            s,
            NewTicket {
                title: title.into(),
                list: Some("todo".into()),
                assignees: assignees.iter().map(|a| a.to_string()).collect(),
                ..Default::default()
            },
            by,
            Utc::now(),
        )
        .unwrap()
        .id
    }

    fn tick() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    fn whys(u: &[Update]) -> Vec<(&str, &str, &str)> {
        u.iter()
            .map(|u| (u.why, u.event.event, u.event.id.as_str()))
            .collect()
    }

    #[test]
    fn reports_what_concerns_me_once_and_never_my_own_changes() {
        let (_d, s) = board();
        let mine = new(&s, "mine", "ada", &["bot"]);
        let theirs = new(&s, "theirs", "ada", &["cy"]);
        assert!(catch_up(&s, "bot").unwrap().is_empty(), "baseline");

        tick();
        let filed = new(&s, "a bug", "ada", &[]);
        ops::move_ticket(&s, &mine, "in-progress", None, "ada", Utc::now()).unwrap();
        ops::add_comment(&s, &mine, "ada", "please check", Utc::now()).unwrap();
        ops::add_comment(&s, &theirs, "ada", "not for bot", Utc::now()).unwrap();
        ops::add_comment(&s, &theirs, "ada", "cc @bot here", Utc::now()).unwrap();
        // Its own change is never news to it.
        ops::move_ticket(&s, &mine, "done", None, "bot", Utc::now()).unwrap();
        tick();

        let u = catch_up(&s, "bot").unwrap();
        assert_eq!(
            whys(&u),
            vec![
                ("new", "created", filed.as_str()),
                ("assigned", "moved", mine.as_str()),
                ("assigned", "commented", mine.as_str()),
                ("mention", "commented", theirs.as_str()),
            ],
            "{u:#?}"
        );
        assert!(catch_up(&s, "bot").unwrap().is_empty(), "reported once");

        // Assignment changes reach the identity both ways.
        ops::update_ticket(
            &s,
            &mine,
            TicketPatch {
                assignees: Some(vec!["cy".into()]),
                ..Default::default()
            },
            "ada",
            Utc::now(),
        )
        .unwrap();
        tick();
        let u = catch_up(&s, "bot").unwrap();
        assert!(
            u.iter()
                .any(|u| u.event.event == "assigned" && u.event.detail == "-bot"),
            "{u:#?}"
        );
    }

    #[test]
    fn mentions_need_a_name_boundary() {
        assert!(mentions("hey @bot, look", "bot"));
        assert!(mentions("@bot", "bot"));
        assert!(!mentions("hey @bottle", "bot"));
        assert!(!mentions("hey @bot-2", "bot"));
    }
}
