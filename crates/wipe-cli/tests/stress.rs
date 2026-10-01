//! Stress and scale: many concurrent `wipe` processes against one board (the
//! multi-agent case), and output volume on large boards (the agent-context case).
//!
//! `output_audit` is `#[ignore]`d: it measures every read/write command on boards
//! of several sizes and writes a Markdown report. Regenerate docs/OUTPUT-AUDIT.md
//! with:
//!   cargo test -p wipe-cli --release --test stress -- --ignored output_audit --nocapture

mod common;

use std::collections::HashSet;
use std::path::Path;
use std::thread;
use std::time::Instant;

use chrono::Utc;
use common::Project;
use wipe_core::ops::{self, NewTicket};
use wipe_core::Store;

/// Run `n_workers` threads, each spawning `per_worker` `wipe` processes built by
/// `args(worker, i)`, all at once. Returns how many processes failed.
fn hammer(
    p: &Project,
    n_workers: usize,
    per_worker: usize,
    args: fn(usize, usize) -> Vec<String>,
) -> usize {
    thread::scope(|s| {
        let handles: Vec<_> = (0..n_workers)
            .map(|w| {
                s.spawn(move || {
                    let mut failed = 0;
                    for i in 0..per_worker {
                        let a = args(w, i);
                        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
                        let mut c = p.cmd(&refs);
                        c.env("WIPE_AUTHOR", format!("agent-{w}"));
                        if !c.output().unwrap().status.success() {
                            failed += 1;
                        }
                    }
                    failed
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    })
}

/// Every ticket file is on exactly one list, and every card has a file.
fn assert_board_consistent(root: &Path) {
    let s = Store::open(root).unwrap();
    let board = s.load_board().unwrap();
    let mut seen = HashSet::new();
    for l in &board.lists {
        for c in &l.cards {
            assert!(seen.insert(c.clone()), "{c} is on more than one list");
            s.load_ticket(c)
                .unwrap_or_else(|_| panic!("card {c} has no ticket file"));
        }
    }
    let files: HashSet<String> = s.ticket_ids().unwrap().into_iter().collect();
    assert_eq!(files, seen, "ticket files and board cards disagree");
    assert!(board.next_ticket as usize > files.len());
}

#[test]
fn concurrent_creates_never_collide_or_lose_tickets() {
    let p = Project::with_board("Race");
    let failed = hammer(&p, 8, 12, |w, i| {
        vec![
            "ticket".into(),
            "create".into(),
            format!("w{w} #{i}"),
            "--list".into(),
            "todo".into(),
        ]
    });
    assert_eq!(failed, 0);
    let s = Store::open(p.path()).unwrap();
    let ids = s.ticket_ids().unwrap();
    assert_eq!(ids.len(), 96, "every create must land exactly once");
    let expected: HashSet<String> = (1..=96).map(|n| format!("T-{n:03X}")).collect();
    assert_eq!(
        ids.into_iter().collect::<HashSet<_>>(),
        expected,
        "ids are dense and unique"
    );
    assert_eq!(
        s.load_board().unwrap().list("todo").unwrap().cards.len(),
        96
    );
    assert_board_consistent(p.path());
}

#[test]
fn concurrent_comments_on_one_ticket_are_all_kept() {
    let p = Project::with_board("Comments");
    p.json(&["ticket", "create", "hot", "--list", "todo"]);
    let failed = hammer(&p, 8, 10, |w, i| {
        vec![
            "comment".into(),
            "add".into(),
            "T-001".into(),
            format!("from {w}: {i}"),
        ]
    });
    assert_eq!(failed, 0);
    let t = Store::open(p.path()).unwrap().load_ticket("T-001").unwrap();
    assert_eq!(t.comments.len(), 80, "no comment may be lost to a race");
    let ids: HashSet<&str> = t.comments.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 80, "comment ids are unique");
    assert_eq!(t.next_comment, 81);
}

#[test]
fn mixed_concurrent_writers_keep_the_board_consistent() {
    let p = Project::with_board("Mixed");
    for i in 0..16 {
        p.json(&[
            "ticket",
            "create",
            &format!("seed {i}"),
            "--list",
            "backlog",
        ]);
    }
    let failed = hammer(&p, 8, 12, |w, i| {
        let t = format!("T-{:03X}", (w * 7 + i) % 16 + 1);
        let lists = ["backlog", "todo", "in-progress", "done"];
        match i % 4 {
            0 => vec![
                "ticket".into(),
                "move".into(),
                t,
                "--to".into(),
                lists[(w + i) % 4].into(),
            ],
            1 => vec![
                "ticket".into(),
                "edit".into(),
                t,
                "--label".into(),
                format!("w{w}"),
                "-m".into(),
                format!("note {i}"),
            ],
            2 => vec![
                "ticket".into(),
                "create".into(),
                format!("new {w}-{i}"),
                "--list".into(),
                "todo".into(),
            ],
            _ => vec![
                "checklist".into(),
                "add".into(),
                t,
                "--text".into(),
                format!("item {w}-{i}"),
            ],
        }
    });
    assert_eq!(failed, 0);
    assert_board_consistent(p.path());
    let s = Store::open(p.path()).unwrap();
    assert_eq!(s.ticket_ids().unwrap().len(), 16 + 8 * 3);
    let checklist_items: usize = s
        .load_all_tickets()
        .unwrap()
        .iter()
        .map(|t| t.checklist.len())
        .sum();
    assert_eq!(checklist_items, 8 * 3);
}

// --- scale ---------------------------------------------------------------------

/// Seed a realistic board directly through wipe-core (fast): `n` tickets spread
/// over the default lists (60% done), each with a ~1 KB body, 5 comments of
/// ~300 bytes, 2 labels, and its activity - the shape the field report described.
fn seed(p: &Project, n: usize) {
    let s = Store::open(p.path()).unwrap();
    let body = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(18);
    let comment = "Checked on device; the flow works but the spinner flickers once. ".repeat(5);
    let now = Utc::now();
    for i in 0..n {
        let list = match i % 10 {
            0..=5 => "done",
            6 => "backlog",
            7 | 8 => "todo",
            _ => "in-progress",
        };
        let t = ops::create_ticket(
            &s,
            NewTicket {
                title: format!("Ticket {i}: a realistic, moderately long title"),
                body: Some(body.clone()),
                list: Some(list.into()),
                labels: vec![
                    "app".into(),
                    if i % 2 == 0 { "backend" } else { "ui" }.into(),
                ],
                ..Default::default()
            },
            "bilal",
            now,
        )
        .unwrap();
        for c in 0..5 {
            ops::add_comment(
                &s,
                &t.id,
                if c % 2 == 0 { "claude" } else { "bilal" },
                &comment,
                now,
            )
            .unwrap();
        }
    }
}

fn bytes_of(p: &Project, args: &[&str]) -> usize {
    p.run(args).len()
}

#[test]
fn compact_outputs_stay_bounded_on_a_large_board() {
    let p = Project::with_board("Big");
    seed(&p, 300);
    // 120 open tickets; compact rows must stay small regardless of history.
    let status = bytes_of(&p, &["status", "--json"]);
    assert!(
        status < 120 * 260,
        "compact status too large: {status} bytes"
    );
    let full = bytes_of(&p, &["status", "--full", "--json"]);
    assert!(
        full > 20 * status,
        "the full dump is the heavy one ({full} vs {status})"
    );

    let todo = bytes_of(&p, &["ticket", "list", "--list", "todo", "--json"]);
    assert!(todo < 60 * 260, "todo listing: {todo} bytes");
    let ids = bytes_of(
        &p,
        &[
            "ticket",
            "list",
            "--exclude-list",
            "done",
            "--fields",
            "id,title",
            "--json",
        ],
    );
    assert!(ids < 120 * 90, "id,title listing: {ids} bytes");

    // Receipts are constant-size no matter how big the ticket is.
    let r = bytes_of(&p, &["comment", "add", "T-001", "x", "--json"]);
    assert!(r < 150, "receipt: {r} bytes");
    let r = bytes_of(&p, &["ticket", "edit", "T-001", "--label", "z", "--json"]);
    assert!(r < 150, "receipt: {r} bytes");

    // The inbox is capped by default (with the true total), and detail snippets
    // are capped per event.
    let inbox = p.json_as("someone-else", &["inbox", "--all"]);
    assert_eq!(inbox["count"], 50);
    assert!(inbox["total"].as_u64().unwrap() > 1000);
    for e in inbox["events"].as_array().unwrap() {
        assert!(e["detail"].as_str().unwrap().chars().count() <= 121);
    }
}

// --- audit -----------------------------------------------------------------------

/// Measure bytes / lines / approximate tokens / wall time for every command an
/// agent routinely runs, at several board sizes, and write a Markdown table to
/// `target/output-audit.md`.
#[test]
#[ignore]
fn output_audit() {
    let sizes = [10usize, 100, 500];
    let cmds: Vec<(&str, Vec<&str>)> = vec![
        ("status (human)", vec!["status"]),
        ("status --json", vec!["status", "--json"]),
        ("status --json --all", vec!["status", "--all", "--json"]),
        ("status --json --full", vec!["status", "--full", "--json"]),
        (
            "status --json --full --pretty (≈0.3 default)",
            vec!["status", "--full", "--json", "--pretty"],
        ),
        (
            "ticket list --list todo --json",
            vec!["ticket", "list", "--list", "todo", "--json"],
        ),
        (
            "ticket list --exclude-list done --json",
            vec!["ticket", "list", "--exclude-list", "done", "--json"],
        ),
        (
            "ticket list --exclude-list done --fields id,title --json",
            vec![
                "ticket",
                "list",
                "--exclude-list",
                "done",
                "--fields",
                "id,title",
                "--json",
            ],
        ),
        (
            "ticket list --full --json",
            vec!["ticket", "list", "--full", "--json"],
        ),
        (
            "ticket show T-001 --json",
            vec!["ticket", "show", "T-001", "--json"],
        ),
        (
            "ticket show T-001 --comments-only --json",
            vec!["ticket", "show", "T-001", "--comments-only", "--json"],
        ),
        (
            "ticket show T-001 --comments 1 --no-activity --json",
            vec![
                "ticket",
                "show",
                "T-001",
                "--comments",
                "1",
                "--no-activity",
                "--json",
            ],
        ),
        ("ticket show T-001 (human)", vec!["ticket", "show", "T-001"]),
        (
            "comment list T-001 --json",
            vec!["comment", "list", "T-001", "--json"],
        ),
        (
            "inbox --all --json (default cap 50)",
            vec!["inbox", "--all", "--json"],
        ),
        (
            "inbox --all --limit 0 --json (uncapped)",
            vec!["inbox", "--all", "--limit", "0", "--json"],
        ),
        (
            "inbox --all --limit 20 --json",
            vec!["inbox", "--all", "--limit", "20", "--json"],
        ),
        ("forum list --json", vec!["forum", "list", "--json"]),
        ("forum digest", vec!["forum", "digest"]),
        (
            "comment add (receipt) --json",
            vec!["comment", "add", "T-002", "ok", "--json"],
        ),
        (
            "comment add --echo --json",
            vec!["comment", "add", "T-002", "ok", "--echo", "--json"],
        ),
        (
            "ticket edit (receipt) --json",
            vec!["ticket", "edit", "T-002", "--label", "x", "--json"],
        ),
        (
            "ticket move (receipt) --json",
            vec!["ticket", "move", "T-002", "--to", "todo", "--json"],
        ),
    ];
    type Cells = Vec<(usize, usize, u128)>; // (bytes, lines, ms) per board size
    let mut rows: Vec<(String, Cells)> = cmds
        .iter()
        .map(|(n, _)| (n.to_string(), Vec::new()))
        .collect();
    for &n in &sizes {
        let p = Project::with_board("Audit");
        seed(&p, n);
        for k in 0..3 {
            p.json(&[
                "forum",
                "post",
                "-t",
                &format!("Rule {k}"),
                "-b",
                &"A durable rule. ".repeat(20),
            ]);
            p.json(&["forum", "pin", &format!("F-{}", k + 1)]);
        }
        for (i, (_, args)) in cmds.iter().enumerate() {
            let mut c = p.cmd(args);
            c.env("WIPE_AUTHOR", "auditor");
            let t0 = Instant::now();
            let out = c.output().unwrap();
            let ms = t0.elapsed().as_millis();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let lines = out.stdout.iter().filter(|b| **b == b'\n').count();
            rows[i].1.push((out.stdout.len(), lines, ms));
        }
    }
    let mut md = String::from("| command | ");
    for n in sizes {
        md.push_str(&format!("{n} tickets: bytes / ~tokens / lines / ms | "));
    }
    md.push_str("\n|---|");
    for _ in sizes {
        md.push_str("---|");
    }
    md.push('\n');
    for (name, cells) in &rows {
        md.push_str(&format!("| `{name}` | "));
        for (b, l, ms) in cells {
            md.push_str(&format!("{b} / {} / {l} / {ms} | ", b / 4));
        }
        md.push('\n');
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/output-audit.md");
    std::fs::write(&target, &md).unwrap();
    println!("{md}");
}
