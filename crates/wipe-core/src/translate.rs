//! One-time translation of a legacy board's decimal ticket IDs (`T-23`) to the
//! hex format (`T-017`).
//!
//! Every place a ticket ID is stored is rewritten consistently: the ticket files
//! (renamed), the board's cards, relations, subscriptions, trash entries, forum
//! refs, and `T-<n>` mentions inside titles, bodies, comments, checklist items
//! and activity. Each ticket keeps its old ID as `legacy_id`, so `T-23` typed
//! later (or found in a commit message) still resolves.
//!
//! Crash safety: the translated tickets are written to a staging directory under
//! the gitignored cache before any original file is removed, so an interrupted
//! run never loses a ticket.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use regex::Regex;

use crate::error::{Error, Result};
use crate::model::{IdFormat, Ticket};
use crate::Store;

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

/// Translate a legacy decimal board to hex ticket IDs. Returns the
/// `(old, new)` pairs in counter order; empty when the board is already hex.
/// Callers must hold the board's write lock.
pub fn translate_ids(store: &Store, now: DateTime<Utc>) -> Result<Vec<(String, String)>> {
    let mut board = store.load_board()?;
    if board.ids == IdFormat::Hex {
        return Ok(Vec::new());
    }
    let tickets = store.load_all_tickets()?;
    let mut trash = crate::trash::read_entries(store)?;

    // old -> new for every ticket on the board and in the trash.
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for id in tickets
        .iter()
        .map(|t| &t.id)
        .chain(trash.iter().map(|e| &e.ticket.id))
    {
        let n: u64 = id
            .strip_prefix("T-")
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| Error::msg(format!("unexpected ticket id `{id}` on a decimal board")))?;
        map.insert(id.clone(), crate::id::hex_ticket_id(n));
    }
    let re = Regex::new(r"\bT-\d+\b").expect("static regex");

    // 1. Stage every translated ticket before touching the originals.
    let stage = store
        .cache_dir()
        .join(format!("translate-{}", now.timestamp()));
    std::fs::create_dir_all(&stage)?;
    let mut translated = Vec::with_capacity(tickets.len());
    for mut t in tickets {
        rewrite_ticket(&mut t, &map, &re);
        let mut json = serde_json::to_string_pretty(&t)?;
        json.push('\n');
        std::fs::write(stage.join(format!("{}.json", t.id)), json)?;
        translated.push(t);
    }

    // 2. Swap: remove the old files, then save the translated ones (old and new
    //    names can coincide from 256 tickets on: decimal `T-100` vs hex `T-100`).
    for old in map.keys() {
        let _ = store.delete_ticket(old);
    }
    for t in &translated {
        store.save_ticket(t)?;
    }

    // 3. The board: cards and the format flag.
    for l in &mut board.lists {
        for c in &mut l.cards {
            if let Some(new) = map.get(c) {
                *c = new.clone();
            }
        }
    }
    board.ids = IdFormat::Hex;
    board.updated = now;
    store.save_board(&board)?;

    // 4. Everything else that names tickets.
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
    if subs_changed {
        store.save_subscriptions(&subs)?;
    }
    for mut thread in store.load_all_threads()? {
        let before = serde_json::to_string(&thread)?;
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
        if serde_json::to_string(&thread)? != before {
            store.save_thread(&thread)?;
        }
    }
    for e in &mut trash {
        let old = e.ticket.id.clone();
        rewrite_ticket(&mut e.ticket, &map, &re);
        crate::trash::rekey_entry(store, &old, e)?;
    }

    let _ = std::fs::remove_dir_all(&stage);
    let mut pairs: Vec<(String, String)> = map.into_iter().collect();
    pairs.sort_by_key(|(old, _)| old[2..].parse::<u64>().unwrap_or(u64::MAX));
    Ok(pairs)
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
}
