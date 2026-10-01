//! One-time translation of a legacy board's decimal ticket IDs (`T-23`) to the
//! hex format (`T-017`).
//!
//! Every place a ticket ID is stored is rewritten consistently: the ticket files
//! (renamed), the board's cards, relations, subscriptions, trash entries, forum
//! refs, and `T-<n>` mentions inside titles, bodies, comments, checklist items
//! and activity. Each ticket keeps its old ID as `legacy_id`, so `T-23` typed
//! later (or found in a commit message) still resolves.
//!
//! Crash safety: the complete result (every file's new content, plus which old
//! files go away) is computed first and written as one journal in the
//! gitignored cache - the commit point. Applying the journal only writes
//! precomputed values and removes files no new one reuses, so it is idempotent:
//! if it is interrupted (a crash, a file held open by another process), the next
//! run - or any CLI command - replays it. `board.json` is written last, so a
//! board only reads as translated once everything else is in place. Boards left
//! half-converted by 0.4.1 (tickets renamed, `board.json` still decimal) are
//! finished the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Board, IdFormat, Subscriptions, Thread, Ticket};
use crate::trash::TrashEntry;
use crate::Store;

const JOURNAL: &str = "translate-journal.json";

/// Everything a translation writes, computed before anything is touched.
#[derive(Serialize, Deserialize)]
struct Journal {
    /// `(old, new)` ids in counter order.
    pairs: Vec<(String, String)>,
    tickets: Vec<Ticket>,
    /// Ticket files to remove: old names that no translated ticket reuses.
    remove_tickets: Vec<String>,
    threads: Vec<Thread>,
    subs: Option<Subscriptions>,
    trash: Vec<TrashEntry>,
    remove_trash: Vec<String>,
    board: Board,
}

/// Rewrite every whole-word `T-<digits>` in `text` that names a translated ticket.
fn rewrite_refs(text: &str, map: &BTreeMap<String, String>, re: &Regex) -> String {
    re.replace_all(text, |c: &regex::Captures| {
        let old = &c[0];
        map.get(old).cloned().unwrap_or_else(|| old.to_string())
    })
    .into_owned()
}

fn rewrite_ticket(t: &mut Ticket, map: &BTreeMap<String, String>, re: &Regex) {
    let r = |s: &str| rewrite_refs(s, map, re);
    if let Some(new) = map.get(&t.id) {
        t.legacy_id = Some(std::mem::replace(&mut t.id, new.clone()));
    }
    t.title = r(&t.title);
    t.body = r(&t.body);
    if let Some(o) = &mut t.original {
        o.title = r(&o.title);
        o.body = r(&o.body);
    }
    for rel in &mut t.relations {
        if let Some(new) = map.get(&rel.target) {
            rel.target = new.clone();
        }
    }
    for c in &mut t.comments {
        c.body = r(&c.body);
    }
    for i in t.checklist.iter_mut().chain(t.acceptance.iter_mut()) {
        i.text = r(&i.text);
    }
    for a in &mut t.activity {
        a.detail = r(&a.detail);
    }
}

fn decimal_counter(id: &str) -> Result<u64> {
    id.strip_prefix("T-")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| Error::msg(format!("unexpected ticket id `{id}` on a decimal board")))
}

/// Whether an interrupted translation is waiting to be finished: a journal,
/// or a decimal board with a 0.4.1 staging directory (a run that renamed the
/// tickets but never got to write `board.json`). Cheap: two directory lookups.
pub fn interrupted(store: &Store) -> bool {
    store.cache_dir().join(JOURNAL).is_file()
        || (!stage_dirs(store).is_empty()
            && store.load_board().is_ok_and(|b| b.ids == IdFormat::Decimal))
}

/// Translate a legacy decimal board to hex ticket IDs, or finish a translation
/// that was interrupted. Returns the `(old, new)` pairs in counter order; empty
/// when the board is already hex. Callers must hold the board's write lock.
pub fn translate_ids(store: &Store, now: DateTime<Utc>) -> Result<Vec<(String, String)>> {
    let journal_path = store.cache_dir().join(JOURNAL);
    if journal_path.is_file() {
        let j: Journal = serde_json::from_slice(&std::fs::read(&journal_path)?)?;
        apply(store, &j).map_err(unfinished)?;
        return Ok(j.pairs);
    }
    let Some(j) = plan(store, now)? else {
        cleanup(store);
        return Ok(Vec::new());
    };
    std::fs::create_dir_all(store.cache_dir())?;
    crate::store::write_json_atomic(&journal_path, &j)?;
    apply(store, &j).map_err(unfinished)?;
    Ok(j.pairs)
}

fn unfinished(e: Error) -> Error {
    Error::msg(format!(
        "ticket-id translation interrupted: {e}. Nothing is lost - it is saved and          finishes on the next wipe command (or `wipe board translate-ids`). If this          repeats, close whatever has the board's files open (an editor, a sync client)"
    ))
}

/// Compute the translation without writing anything (`None` if already hex).
fn plan(store: &Store, now: DateTime<Utc>) -> Result<Option<Journal>> {
    let mut board = store.load_board()?;
    if board.ids == IdFormat::Hex {
        return Ok(None);
    }
    let on_disk = store.ticket_ids()?;

    // Tickets that already carry a legacy_id were translated by an interrupted
    // 0.4.1 run: keep them as they are. Its staging snapshot (complete, taken
    // under the lock) supplies any it deleted but never wrote back.
    let mut done: BTreeMap<String, Ticket> = BTreeMap::new();
    let mut todo: Vec<Ticket> = Vec::new();
    for t in store.load_all_tickets()? {
        if t.legacy_id.is_some() {
            done.insert(t.id.clone(), t);
        } else {
            todo.push(t);
        }
    }
    for t in staged_tickets(store) {
        if t.legacy_id.is_some() && !done.contains_key(&t.id) {
            done.insert(t.id.clone(), t);
        }
    }
    let already: BTreeSet<String> = done.values().filter_map(|t| t.legacy_id.clone()).collect();
    // A leftover old file of a ticket that was already translated is stale.
    todo.retain(|t| !already.contains(&t.id));

    let trash_on_disk = crate::trash::read_entries(store)?;
    let trash_ids: Vec<String> = trash_on_disk.iter().map(|e| e.ticket.id.clone()).collect();
    let (trash_done, trash_todo): (Vec<TrashEntry>, Vec<TrashEntry>) = trash_on_disk
        .into_iter()
        .partition(|e| e.ticket.legacy_id.is_some());

    // old -> new for every ticket on the board and in the trash.
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for t in done.values().chain(trash_done.iter().map(|e| &e.ticket)) {
        if let Some(old) = &t.legacy_id {
            map.insert(old.clone(), t.id.clone());
        }
    }
    for id in todo
        .iter()
        .map(|t| &t.id)
        .chain(trash_todo.iter().map(|e| &e.ticket.id))
    {
        map.insert(id.clone(), crate::id::hex_ticket_id(decimal_counter(id)?));
    }
    let mut seen = BTreeSet::new();
    if let Some(dup) = map.values().find(|new| !seen.insert(*new)) {
        return Err(Error::msg(format!(
            "cannot translate: two tickets would both become `{dup}`"
        )));
    }
    let re = Regex::new(r"\bT-\d+\b").expect("static regex");

    let mut tickets: Vec<Ticket> = done.into_values().collect();
    for mut t in todo {
        rewrite_ticket(&mut t, &map, &re);
        tickets.push(t);
    }
    let keep: BTreeSet<&str> = tickets.iter().map(|t| t.id.as_str()).collect();
    let remove_tickets = on_disk
        .into_iter()
        .filter(|id| !keep.contains(id.as_str()))
        .collect();

    let mut trash = trash_done;
    for mut e in trash_todo {
        rewrite_ticket(&mut e.ticket, &map, &re);
        trash.push(e);
    }
    let keep: BTreeSet<&str> = trash.iter().map(|e| e.ticket.id.as_str()).collect();
    let remove_trash = trash_ids
        .into_iter()
        .filter(|id| !keep.contains(id.as_str()))
        .collect();

    for l in &mut board.lists {
        for c in &mut l.cards {
            if let Some(new) = map.get(c) {
                *c = new.clone();
            }
        }
    }
    board.ids = IdFormat::Hex;
    board.updated = now;

    let mut subs = store.load_subscriptions()?;
    let mut subs_changed = false;
    for refs in subs.subs.values_mut() {
        for r in refs.iter_mut() {
            if let Some(new) = map.get(r) {
                *r = new.clone();
                subs_changed = true;
            }
        }
    }

    let mut threads = Vec::new();
    for mut thread in store.load_all_threads()? {
        let before = thread.clone();
        thread.title = rewrite_refs(&thread.title, &map, &re);
        let mut stack = vec![&mut thread.root];
        while let Some(p) = stack.pop() {
            p.body = rewrite_refs(&p.body, &map, &re);
            for r in &mut p.refs {
                if let Some(new) = map.get(r) {
                    *r = new.clone();
                }
            }
            stack.extend(p.replies.iter_mut());
        }
        if thread != before {
            threads.push(thread);
        }
    }

    let mut pairs: Vec<(String, String)> = map.into_iter().collect();
    pairs.sort_by_key(|(old, _)| decimal_counter(old).unwrap_or(u64::MAX));
    Ok(Some(Journal {
        pairs,
        tickets,
        remove_tickets,
        threads,
        subs: subs_changed.then_some(subs),
        trash,
        remove_trash,
        board,
    }))
}

/// Write the journal's values: new files first, then the removal of old names
/// no new file reuses (decimal `T-100` and hex `T-100` coincide), `board.json`
/// last. Safe to repeat after any interruption.
fn apply(store: &Store, j: &Journal) -> Result<()> {
    for t in &j.tickets {
        store.save_ticket(t)?;
    }
    for id in &j.remove_tickets {
        store.remove_ticket_file(id)?;
    }
    for t in &j.threads {
        store.save_thread(t)?;
    }
    if let Some(subs) = &j.subs {
        store.save_subscriptions(subs)?;
    }
    for e in &j.trash {
        crate::trash::write_entry(store, e)?;
    }
    for id in &j.remove_trash {
        crate::trash::remove_entry_file(store, id)?;
    }
    store.save_board(&j.board)?;
    std::fs::remove_file(store.cache_dir().join(JOURNAL))?;
    cleanup(store);
    Ok(())
}

/// Tickets in 0.4.1's staging directories (`.cache/translate-<ts>/`).
fn staged_tickets(store: &Store) -> Vec<Ticket> {
    stage_dirs(store)
        .into_iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .filter_map(|f| std::fs::read(f.path()).ok())
        .filter_map(|b| serde_json::from_slice::<Ticket>(&b).ok())
        .collect()
}

fn stage_dirs(store: &Store) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(store.cache_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("translate-"))
        })
        .collect();
    dirs.sort();
    dirs
}

fn cleanup(store: &Store) {
    for d in stage_dirs(store) {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Board, Relation, RelationKind};
    use crate::ops::{self, NewTicket};

    /// A board as 0.4.0 wrote it: decimal ids, no `ids` field.
    fn legacy_board(n: usize) -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::init(dir.path(), "Legacy", Utc::now()).unwrap();
        let mut b: Board = s.load_board().unwrap();
        b.ids = IdFormat::Decimal;
        s.save_board(&b).unwrap();
        for i in 0..n {
            ops::create_ticket(
                &s,
                NewTicket {
                    title: format!("ticket {i}"),
                    list: Some("todo".into()),
                    ..Default::default()
                },
                "ada",
                Utc::now(),
            )
            .unwrap();
        }
        (dir, s)
    }

    #[test]
    fn translates_ids_everywhere_and_keeps_legacy_aliases() {
        let (_d, s) = legacy_board(300);
        assert_eq!(s.load_board().unwrap().lists[1].cards[9], "T-10");
        // References in text, relations, subscriptions, forum, trash.
        let mut t = s.load_ticket("T-3").unwrap();
        t.body = "see T-10 and T-256, not T-1000 or XT-10".into();
        t.relations.push(Relation {
            kind: RelationKind::BlockedBy,
            target: "T-100".into(),
        });
        s.save_ticket(&t).unwrap();
        ops::add_comment(&s, "T-3", "ada", "follow-up in T-299", Utc::now()).unwrap();
        crate::inbox::subscribe(&s, "ada", "T-256").unwrap();
        crate::forum::create_thread(
            &s,
            crate::forum::NewThread {
                title: "about T-12".into(),
                body: "T-12 is done".into(),
                refs: vec!["T-12".into()],
                ..Default::default()
            },
            "ada",
            Utc::now(),
        )
        .unwrap();
        crate::trash::trash_ticket(&s, "T-300", 7, Utc::now()).unwrap();

        let pairs = translate_ids(&s, Utc::now()).unwrap();
        assert_eq!(pairs.len(), 300);
        assert_eq!(pairs[0], ("T-1".into(), "T-001".into()));
        assert_eq!(pairs[255], ("T-256".into(), "T-100".into()));

        let b = s.load_board().unwrap();
        assert_eq!(b.ids, IdFormat::Hex);
        assert_eq!(b.lists[1].cards[9], "T-00A");
        assert_eq!(b.lists[1].cards.len(), 299);
        // Old T-100 and new T-100 (=256) did not clobber each other.
        assert_eq!(s.load_ticket("T-064").unwrap().title, "ticket 99");
        assert_eq!(s.load_ticket("T-100").unwrap().title, "ticket 255");

        let t = s.load_ticket("T-003").unwrap();
        assert_eq!(t.legacy_id.as_deref(), Some("T-3"));
        assert_eq!(t.body, "see T-00A and T-100, not T-1000 or XT-10");
        assert_eq!(t.relations[0].target, "T-064");
        assert_eq!(t.comments[0].body, "follow-up in T-12B");
        assert_eq!(
            crate::inbox::subscriptions_of(&s, "ada").unwrap(),
            vec!["T-100"]
        );
        let th = s.load_thread("F-1").unwrap();
        assert_eq!(
            (th.title.as_str(), th.root.refs[0].as_str()),
            ("about T-00C", "T-00C")
        );
        let trashed = crate::trash::list_trash(&s, 7, Utc::now()).unwrap();
        assert_eq!(trashed[0].ticket.id, "T-12C");
        crate::trash::restore_ticket(&s, "T-12C", Utc::now()).unwrap();

        // Every way of naming a ticket still resolves.
        for (typed, want) in [
            ("T-003", "T-003"),
            ("t3", "T-003"),
            ("T-3", "T-003"),   // legacy decimal, short
            ("T-82", "T-052"),  // legacy decimal 82, not hex 0x82
            ("T-082", "T-082"), // padded: canonical hex
            ("T-2a", "T-02A"),
            ("T-100", "T-100"), // canonical wins over legacy T-100 (= T-064)
        ] {
            assert_eq!(s.resolve_ticket_id(typed).unwrap(), want, "{typed}");
        }
        // New tickets continue the counter in hex; a second run is a no-op.
        let n = ops::create_ticket(
            &s,
            NewTicket {
                title: "new".into(),
                list: Some("todo".into()),
                ..Default::default()
            },
            "ada",
            Utc::now(),
        )
        .unwrap();
        assert_eq!(n.id, "T-12D");
        assert!(translate_ids(&s, Utc::now()).unwrap().is_empty());
        assert_eq!(s.ticket_ids().unwrap()[..3], ["T-001", "T-002", "T-003"]);
    }

    #[test]
    fn decimal_boards_keep_working_until_translated() {
        let (_d, s) = legacy_board(12);
        assert_eq!(s.resolve_ticket_id("t12").unwrap(), "T-12");
        assert!(s.resolve_ticket_id("T-13").is_err());
        let ids = s.ticket_ids().unwrap();
        assert_eq!(&ids[8..], ["T-9", "T-10", "T-11", "T-12"]);
    }

    /// A small legacy board with cross-references in a ticket, the forum and
    /// subscriptions.
    fn legacy_board_with_refs() -> (tempfile::TempDir, Store) {
        let (d, s) = legacy_board(12);
        let mut t = s.load_ticket("T-3").unwrap();
        t.body = "see T-10".into();
        s.save_ticket(&t).unwrap();
        crate::inbox::subscribe(&s, "ada", "T-11").unwrap();
        crate::forum::create_thread(
            &s,
            crate::forum::NewThread {
                title: "about T-12".into(),
                body: "T-12".into(),
                refs: vec!["T-12".into()],
                ..Default::default()
            },
            "ada",
            Utc::now(),
        )
        .unwrap();
        (d, s)
    }

    fn assert_fully_translated(s: &Store) {
        let b = s.load_board().unwrap();
        assert_eq!(b.ids, IdFormat::Hex);
        assert_eq!(b.lists[1].cards[9], "T-00A");
        assert_eq!(s.ticket_ids().unwrap().len(), 12);
        for id in s.ticket_ids().unwrap() {
            assert_eq!(id.len(), 5, "stale file {id}");
        }
        for c in b.lists.iter().flat_map(|l| &l.cards) {
            s.load_ticket(c).unwrap();
        }
        assert_eq!(s.load_ticket("T-003").unwrap().body, "see T-00A");
        assert_eq!(
            s.load_ticket("T-00C").unwrap().legacy_id.as_deref(),
            Some("T-12")
        );
        assert_eq!(
            crate::inbox::subscriptions_of(s, "ada").unwrap(),
            vec!["T-00B"]
        );
        assert_eq!(s.load_thread("F-1").unwrap().root.refs[0], "T-00C");
        assert!(!interrupted(s));
        assert!(stage_dirs(s).is_empty());
        assert!(!s.cache_dir().join(JOURNAL).exists());
    }

    /// Replays what 0.4.1 left behind when `board.json` could not be written:
    /// a staging dir, every ticket renamed, everything else still decimal.
    #[test]
    fn finishes_a_board_half_converted_by_0_4_1() {
        let (_d, s) = legacy_board_with_refs();
        let j = plan(&s, Utc::now()).unwrap().unwrap();
        let stage = s.cache_dir().join("translate-1790875456");
        std::fs::create_dir_all(&stage).unwrap();
        for t in &j.tickets {
            std::fs::write(
                stage.join(format!("{}.json", t.id)),
                serde_json::to_string(t).unwrap(),
            )
            .unwrap();
        }
        for id in &j.remove_tickets {
            s.remove_ticket_file(id).unwrap();
        }
        for t in &j.tickets {
            s.save_ticket(t).unwrap();
        }
        assert_eq!(s.load_board().unwrap().ids, IdFormat::Decimal);
        assert!(interrupted(&s));

        // Before 0.4.2 this failed: "unexpected ticket id `T-001` on a decimal board".
        let pairs = translate_ids(&s, Utc::now()).unwrap();
        assert_eq!(pairs.len(), 12);
        assert_eq!(pairs[11], ("T-12".into(), "T-00C".into()));
        assert_fully_translated(&s);
    }

    /// 0.4.1 killed mid-swap: some old files deleted, few new ones written;
    /// the staging snapshot fills the gap.
    #[test]
    fn recovers_tickets_only_present_in_the_0_4_1_staging_dir() {
        let (_d, s) = legacy_board_with_refs();
        let j = plan(&s, Utc::now()).unwrap().unwrap();
        let stage = s.cache_dir().join("translate-1");
        std::fs::create_dir_all(&stage).unwrap();
        for t in &j.tickets {
            std::fs::write(
                stage.join(format!("{}.json", t.id)),
                serde_json::to_string(t).unwrap(),
            )
            .unwrap();
        }
        for id in &j.remove_tickets[..8] {
            s.remove_ticket_file(id).unwrap();
        }
        s.save_ticket(&j.tickets[0]).unwrap();

        translate_ids(&s, Utc::now()).unwrap();
        assert_fully_translated(&s);
    }

    /// A run interrupted while applying its journal is replayed, not redone
    /// (redoing would re-map refs that are already translated).
    #[test]
    fn replays_an_interrupted_journal() {
        let (_d, s) = legacy_board_with_refs();
        let j = plan(&s, Utc::now()).unwrap().unwrap();
        crate::store::write_json_atomic(&s.cache_dir().join(JOURNAL), &j).unwrap();
        for t in &j.tickets[..5] {
            s.save_ticket(t).unwrap();
        }
        for id in &j.remove_tickets[..3] {
            s.remove_ticket_file(id).unwrap();
        }
        assert!(interrupted(&s));
        assert_eq!(translate_ids(&s, Utc::now()).unwrap().len(), 12);
        assert_fully_translated(&s);
    }

    /// Old `T-100` and new `T-100` (= 256) in the trash must not clobber each other.
    #[test]
    fn trash_entries_with_colliding_names_both_survive() {
        let (_d, s) = legacy_board(260);
        crate::trash::trash_ticket(&s, "T-100", 7, Utc::now()).unwrap();
        crate::trash::trash_ticket(&s, "T-256", 7, Utc::now()).unwrap();
        translate_ids(&s, Utc::now()).unwrap();
        let mut ids: Vec<String> = crate::trash::list_trash(&s, 7, Utc::now())
            .unwrap()
            .into_iter()
            .map(|e| e.ticket.id)
            .collect();
        ids.sort();
        assert_eq!(ids, ["T-064", "T-100"]);
        assert_eq!(
            s.load_board().unwrap().lists[1].cards.len(),
            258,
            "trashed tickets stay off the board"
        );
    }
}
