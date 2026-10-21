//! Board change events: what `wipe watch` streams and how activity reads as one
//! condensed line.
//!
//! A [`Watcher`] snapshots the board (each ticket's list, comment ids, activity
//! length, plus the forum's post ids) and, on every [`Watcher::poll`], turns the
//! difference into [`Event`]s: `created`, `moved` (with the source and target
//! list), `commented`, `edited`, `labeled`, `assigned`, `blocked`, `attached`,
//! `checked`, `deleted`, and forum `posted`. It diffs by count rather than by
//! timestamp, so changes that arrive through `git pull` (with older timestamps)
//! are reported too. Polling only stats files until something changed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::Ticket;
use crate::{forum, Store};

/// One change on the board.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    /// When it happened (the activity / comment / post time when known).
    pub ts: DateTime<Utc>,
    /// `created`, `moved`, `commented`, `edited`, `labeled`, `assigned`,
    /// `blocked`, `attached`, `checked`, `deleted`, or `posted` (forum).
    pub event: &'static str,
    /// The ticket (`T-02A`) or forum post (`F-3.2`) it happened on.
    pub id: String,
    /// The ticket or thread title.
    pub title: String,
    /// Who did it (empty when unknown, e.g. a file deleted outside wipe).
    pub by: String,
    /// The ticket's list after the change (`None` once deleted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    /// For `moved`: the list it left.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Short detail: a comment snippet, `+bug`, `title`, `ac-2 Runs on CI`...
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// The ticket's assignees after the change - before it, for `deleted` - so
    /// readers can tell whose ticket it is. Not part of `watch`'s output.
    #[serde(skip)]
    pub assignees: Vec<String>,
}

/// The event kind and detail an activity entry reads as. Unknown kinds pass
/// through unchanged, so newer activity never disappears from older readers.
pub fn describe(kind: &str, detail: &str) -> (&'static str, String) {
    let d = detail.trim();
    match kind {
        "created" => ("created", String::new()),
        "moved" => ("moved", d.to_string()),
        "renamed" => ("edited", format!("title: {d}")),
        "edited" => ("edited", "body".into()),
        "priority" => (
            "edited",
            format!("priority: {}", if d.is_empty() { "none" } else { d }),
        ),
        "label-added" => ("labeled", format!("+{d}")),
        "label-removed" => ("labeled", format!("-{d}")),
        "assigned" => ("assigned", format!("+{d}")),
        "unassigned" => ("assigned", format!("-{d}")),
        "blocked-by" => ("blocked", format!("by {d}")),
        "unblocked" => ("blocked", format!("no longer by {d}")),
        "attached" => ("attached", format!("+{d}")),
        "detached" => ("attached", format!("-{d}")),
        "checked" => ("checked", format!("[x] {d}")),
        "unchecked" => ("checked", format!("[ ] {d}")),
        "reattributed" | "comment-reattributed" => ("edited", d.to_string()),
        _ => ("edited", format!("{kind} {d}").trim().to_string()),
    }
}

/// Every event kind, for validating filters.
pub const KINDS: &[&str] = &[
    "created",
    "moved",
    "commented",
    "edited",
    "labeled",
    "assigned",
    "blocked",
    "attached",
    "checked",
    "deleted",
    "posted",
];

/// Collapse whitespace and clip to `n` characters (with an ellipsis).
pub fn snippet(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n {
        one
    } else {
        format!("{}…", one.chars().take(n).collect::<String>().trim_end())
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct TicketSnap {
    list: Option<String>,
    title: String,
    comments: HashSet<String>,
    activity: usize,
    #[serde(default)]
    assignees: Vec<String>,
}

impl TicketSnap {
    fn of(t: &Ticket, list: Option<String>) -> Self {
        TicketSnap {
            list,
            title: t.title.clone(),
            comments: t.comments.iter().map(|c| c.id.clone()).collect(),
            activity: t.activity.len(),
            assignees: t.assignees.clone(),
        }
    }
}

/// Each board file's (modified time in nanoseconds, size).
type Stamp = HashMap<PathBuf, (u64, u64)>;

/// What a [`Watcher`] remembers about the board: enough to diff the next look
/// against. Serializable, so a snapshot can outlive the process (see
/// [`crate::aware`]).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct WatchState {
    stamp: Stamp,
    tickets: HashMap<String, TicketSnap>,
    posts: HashSet<String>,
}

/// Polls a board for changes and reports them as [`Event`]s.
pub struct Watcher {
    store: Store,
    state: WatchState,
    dirty: bool,
}

fn file_stamp(m: &std::fs::Metadata) -> (u64, u64) {
    let nanos = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    (nanos, m.len())
}

fn stamp(store: &Store) -> Stamp {
    let mut out = Stamp::new();
    let board = store.wipe_dir().join("board.json");
    for dir in [store.tickets_dir(), store.forum_dir()] {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if let Ok(m) = e.metadata() {
                out.insert(e.path(), file_stamp(&m));
            }
        }
    }
    if let Ok(m) = std::fs::metadata(&board) {
        out.insert(board, file_stamp(&m));
    }
    out
}

fn card_lists(store: &Store) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Ok(b) = store.load_board() {
        for l in b.lists {
            for c in l.cards {
                out.insert(c, l.id.clone());
            }
        }
    }
    out
}

impl Watcher {
    /// Snapshot the board as it is now; only later changes are reported.
    pub fn new(store: Store) -> Result<Self> {
        let stamp = stamp(&store);
        let lists = card_lists(&store);
        let mut tickets = HashMap::new();
        for t in store.load_all_tickets()? {
            let list = lists.get(&t.id).cloned();
            tickets.insert(t.id.clone(), TicketSnap::of(&t, list));
        }
        let posts = forum::index(&store)?.into_iter().map(|p| p.id).collect();
        Ok(Watcher {
            store,
            state: WatchState {
                stamp,
                tickets,
                posts,
            },
            dirty: true,
        })
    }

    /// Continue from a snapshot taken earlier (by [`Watcher::state`]): the next
    /// poll reports everything that changed since then.
    pub fn resume(store: Store, state: WatchState) -> Self {
        Watcher {
            store,
            state,
            dirty: false,
        }
    }

    /// The current snapshot, to [`Watcher::resume`] from later.
    pub fn state(&self) -> &WatchState {
        &self.state
    }

    /// Whether the snapshot changed since it was taken or resumed.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The board being watched.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Changes since the last poll, oldest first. Cheap when nothing changed:
    /// it only stats the board's files.
    pub fn poll(&mut self) -> Vec<Event> {
        let now_stamp = stamp(&self.store);
        if now_stamp == self.state.stamp {
            return Vec::new();
        }
        self.dirty = true;
        let state = &mut self.state;
        let tickets_dir = self.store.tickets_dir();
        let forum_dir = self.store.forum_dir();
        let changed = |p: &PathBuf| state.stamp.get(p) != now_stamp.get(p);
        let mut ticket_changed: HashSet<String> = HashSet::new();
        let mut forum_changed = false;
        for p in now_stamp.keys().chain(state.stamp.keys()) {
            if !changed(p) {
                continue;
            }
            if p.parent() == Some(tickets_dir.as_path()) {
                if let Some(id) = p.file_stem().and_then(|s| s.to_str()) {
                    if p.extension().is_some_and(|e| e == "json") {
                        ticket_changed.insert(id.to_string());
                    }
                }
            } else if p.parent() == Some(forum_dir.as_path()) {
                forum_changed = true;
            }
        }
        let board_changed = changed(&self.store.wipe_dir().join("board.json"));
        let lists = card_lists(&self.store);
        let mut events = Vec::new();

        // A card that changed lists without its ticket file changing (a pulled
        // board.json): still a move.
        if board_changed {
            for (id, snap) in &state.tickets {
                if !ticket_changed.contains(id) && lists.get(id) != snap.list.as_ref() {
                    ticket_changed.insert(id.clone());
                }
            }
        }
        let mut ids: Vec<String> = ticket_changed.into_iter().collect();
        ids.sort();
        for id in ids {
            let prev = state.tickets.get(&id).cloned();
            match self.store.load_ticket(&id) {
                Ok(t) if t.id == id => {
                    let list = lists.get(&id).cloned();
                    ticket_events(&t, prev.as_ref(), list.clone(), &mut events);
                    state.tickets.insert(id, TicketSnap::of(&t, list));
                }
                Ok(_) => {}
                Err(_) => {
                    // Gone (deleted, or mid-rename by an id translation).
                    if let Some(p) = prev {
                        if !now_stamp.contains_key(&tickets_dir.join(format!("{id}.json"))) {
                            events.push(Event {
                                ts: Utc::now(),
                                event: "deleted",
                                id: id.clone(),
                                title: p.title.clone(),
                                by: String::new(),
                                list: None,
                                from: p.list.clone(),
                                detail: String::new(),
                                assignees: p.assignees.clone(),
                            });
                            state.tickets.remove(&id);
                        }
                    }
                }
            }
        }
        if forum_changed {
            if let Ok(all) = forum::index(&self.store) {
                let mut fresh: Vec<_> = all
                    .iter()
                    .filter(|p| !state.posts.contains(&p.id))
                    .collect();
                fresh.sort_by_key(|p| p.created);
                for p in fresh {
                    events.push(Event {
                        ts: p.created,
                        event: "posted",
                        id: p.id.clone(),
                        title: p.thread_title.clone(),
                        by: p.author.clone(),
                        list: None,
                        from: None,
                        detail: snippet(&p.body, 160),
                        assignees: Vec::new(),
                    });
                }
                state.posts = all.into_iter().map(|p| p.id).collect();
            }
        }
        state.stamp = now_stamp;
        events.sort_by_key(|e| e.ts);
        events
    }
}

fn ticket_events(
    t: &Ticket,
    prev: Option<&TicketSnap>,
    list: Option<String>,
    out: &mut Vec<Event>,
) {
    let ev = |ts, event, by: &str, detail: String| Event {
        ts,
        event,
        id: t.id.clone(),
        title: t.title.clone(),
        by: by.to_string(),
        list: list.clone(),
        from: None,
        detail,
        assignees: t.assignees.clone(),
    };
    let Some(prev) = prev else {
        let by = t.creator().unwrap_or_default();
        out.push(ev(t.created, "created", by, String::new()));
        return;
    };
    let mut moved_logged = false;
    for a in t.activity.iter().skip(prev.activity) {
        let (event, detail) = describe(&a.kind, &a.detail);
        if event == "created" {
            continue;
        }
        let mut e = ev(a.ts, event, &a.actor, detail);
        if event == "moved" {
            moved_logged = true;
            e.from = prev.list.clone();
            e.detail = String::new();
        }
        out.push(e);
    }
    if !moved_logged && list != prev.list && list.is_some() {
        let mut e = ev(t.updated, "moved", "", String::new());
        e.from = prev.list.clone();
        out.push(e);
    }
    for c in t.comments.iter().filter(|c| !prev.comments.contains(&c.id)) {
        out.push(ev(c.created, "commented", &c.author, snippet(&c.body, 200)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{self, NewTicket};

    fn board() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::init(d.path(), "W", Utc::now()).unwrap();
        (d, s)
    }

    fn new(s: &Store, title: &str) -> String {
        ops::create_ticket(
            s,
            NewTicket {
                title: title.into(),
                list: Some("todo".into()),
                ..Default::default()
            },
            "ada",
            Utc::now(),
        )
        .unwrap()
        .id
    }

    #[test]
    fn reports_each_kind_of_change_once_with_its_actor() {
        let (_d, s) = board();
        let id = new(&s, "first");
        let mut w = Watcher::new(s.clone()).unwrap();
        assert!(w.poll().is_empty());

        // Distinct mtimes on coarse filesystems.
        let tick = || std::thread::sleep(std::time::Duration::from_millis(20));
        ops::move_ticket(&s, &id, "in-progress", None, "bob", Utc::now()).unwrap();
        ops::add_comment(&s, &id, "bob", "looks   good\nship it", Utc::now()).unwrap();
        tick();
        let e = w.poll();
        assert_eq!(e.len(), 2, "{e:?}");
        assert_eq!(
            (
                e[0].event,
                e[0].from.as_deref(),
                e[0].list.as_deref(),
                e[0].by.as_str()
            ),
            ("moved", Some("todo"), Some("in-progress"), "bob")
        );
        assert_eq!(
            (e[1].event, e[1].detail.as_str()),
            ("commented", "looks good ship it")
        );
        assert!(w.poll().is_empty(), "nothing new on a second poll");

        let id2 = new(&s, "second");
        ops::update_ticket(
            &s,
            &id,
            ops::TicketPatch {
                labels: Some(vec!["bug".into()]),
                ..Default::default()
            },
            "cy",
            Utc::now(),
        )
        .unwrap();
        tick();
        let e = w.poll();
        let kinds: Vec<_> = e
            .iter()
            .map(|e| (e.event, e.id.as_str(), e.detail.as_str()))
            .collect();
        assert!(kinds.contains(&("created", id2.as_str(), "")), "{kinds:?}");
        assert!(
            kinds.contains(&("labeled", id.as_str(), "+bug")),
            "{kinds:?}"
        );

        crate::trash::trash_ticket(&s, &id2, 7, Utc::now()).unwrap();
        tick();
        let e = w.poll();
        assert_eq!(
            (e[0].event, e[0].id.as_str(), e[0].from.as_deref()),
            ("deleted", id2.as_str(), Some("todo"))
        );
    }

    #[test]
    fn a_pulled_board_move_without_activity_is_still_a_move() {
        let (_d, s) = board();
        let id = new(&s, "x");
        let mut w = Watcher::new(s.clone()).unwrap();
        let mut b = s.load_board().unwrap();
        for l in &mut b.lists {
            l.cards.retain(|c| c != &id);
        }
        b.list_mut("done").unwrap().cards.push(id.clone());
        s.save_board(&b).unwrap();
        let e = w.poll();
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(
            (e[0].event, e[0].from.as_deref(), e[0].list.as_deref()),
            ("moved", Some("todo"), Some("done"))
        );
    }
}
