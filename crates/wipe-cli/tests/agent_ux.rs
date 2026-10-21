//! The agent-experience contract: text input via files/stdin, small receipts,
//! compact and bounded views with hints, search, the review loop, dependencies,
//! the forum digest, `watch`, output guardrails, and hex ticket ids. Each test
//! drives the real binary.

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::time::Duration;

use common::Project;
use serde_json::{json, Value};

fn ids(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect()
}

// --- text input -------------------------------------------------------------------

#[test]
fn multiline_bodies_survive_via_file_and_stdin() {
    let p = Project::with_board("Text");
    let body = "line one\n\nline three with \"quotes\" and $dollars\n- a list";

    // A BOM and trailing newlines (editor/PowerShell artifacts) are not content.
    std::fs::write(p.path().join("b.md"), format!("\u{feff}{body}\n\n")).unwrap();
    p.json(&[
        "ticket",
        "create",
        "From file",
        "--list",
        "todo",
        "--body-file",
        "b.md",
    ]);
    assert_eq!(p.json(&["ticket", "show", "T-001"])["body"], body);
    let out = p.with_stdin(
        &[
            "ticket",
            "create",
            "From stdin",
            "--list",
            "todo",
            "--body",
            "-",
            "--json",
        ],
        &format!("{body}\r\n"),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(p.json(&["ticket", "show", "T-002"])["body"], body);

    // Comments: positional, file and stdin forms agree.
    p.json(&["comment", "add", "T-001", "positional comment"]);
    std::fs::write(p.path().join("c.md"), "multi\nline\ncomment").unwrap();
    p.json(&["comment", "add", "T-001", "--body-file", "c.md"]);
    assert!(p
        .with_stdin(
            &["comment", "add", "T-001", "-b", "-", "--json"],
            "from\nstdin"
        )
        .status
        .success());
    let bodies: Vec<String> = p.json(&["comment", "list", "T-001"])["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["body"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        bodies,
        ["positional comment", "multi\nline\ncomment", "from\nstdin"]
    );

    // Forum post, reply and edit take files too.
    p.json(&["forum", "post", "-t", "Notes", "--body-file", "c.md"]);
    p.json(&["forum", "reply", "F-1", "--body-file", "b.md"]);
    p.json(&["forum", "edit", "F-1", "--body-file", "b.md"]);
    let t = p.json(&["forum", "show", "F-1"]);
    assert_eq!(
        (
            t["post"]["body"].as_str(),
            t["post"]["replies"][0]["body"].as_str()
        ),
        (Some(body), Some(body))
    );

    // A missing body source fails with guidance, not an empty comment.
    let e = p.json_err(&["comment", "add", "T-001"]);
    assert!(e["error"].as_str().unwrap().contains("--body-file"), "{e}");
    let e = p.json_err(&[
        "ticket",
        "create",
        "X",
        "--list",
        "todo",
        "--body-file",
        "nope.md",
    ]);
    assert!(e["error"].as_str().unwrap().contains("nope.md"), "{e}");
}

// --- output size ------------------------------------------------------------------

#[test]
fn writes_return_short_receipts_unless_echo() {
    let p = Project::with_board("Receipts");
    p.json(&[
        "ticket",
        "create",
        "A",
        "--list",
        "todo",
        "--body",
        "some body",
    ]);
    for i in 0..5 {
        p.json(&[
            "comment",
            "add",
            "T-001",
            &format!("comment {i} with some length to it"),
        ]);
    }
    let receipt = p.run(&["comment", "add", "T-001", "one more", "--json"]);
    let v: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        (v["ok"].as_bool(), v["comment"].as_str()),
        (Some(true), Some("c-6"))
    );
    assert!(
        receipt.len() < 200 && receipt.trim().lines().count() == 1,
        "{receipt}"
    );

    let full = p.json(&["--echo", "comment", "add", "T-001", "echoed"]);
    assert_eq!(
        full["comments"].as_array().unwrap().len(),
        7,
        "--echo returns the whole ticket"
    );
    assert_eq!(
        p.json(&["ticket", "move", "T-001", "--to", "done"]),
        json!({ "ok": true, "id": "T-001", "list": "done" })
    );
    assert!(
        p.run(&["ticket", "show", "T-001", "--json", "--pretty"])
            .lines()
            .count()
            > 10
    );
}

#[test]
fn status_is_compact_and_collapses_done() {
    let p = Project::with_board("Status");
    let long = "x".repeat(4000);
    p.json(&[
        "ticket", "create", "Open", "--list", "todo", "--body", &long, "--label", "app",
    ]);
    for i in 0..3 {
        p.json(&["ticket", "create", &format!("Done {i}"), "--list", "done"]);
    }
    for _ in 0..3 {
        p.json(&["comment", "add", "T-001", &long]);
    }

    let raw = p.run(&["status", "--json"]);
    assert!(
        raw.len() < 700,
        "status carries no bodies or comments ({} bytes)",
        raw.len()
    );
    let s: Value = serde_json::from_str(&raw).unwrap();
    let lists = s["lists"].as_array().unwrap();
    let todo = lists.iter().find(|l| l["id"] == "todo").unwrap();
    assert_eq!(
        todo["tickets"][0],
        json!({ "id": "T-001", "title": "Open", "labels": ["app"] })
    );
    let done = lists.iter().find(|l| l["id"] == "done").unwrap();
    assert_eq!(
        (done["count"].as_u64(), done["collapsed"].as_bool()),
        (Some(3), Some(true))
    );
    assert!(done.get("tickets").is_none());
    assert_eq!(s["identity"], "Tester <t@example.com>");

    // Detail is opt-in.
    let more = p.json(&["status", "--fields", "id,comments,updated"]);
    let row = &more["lists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == "todo")
        .unwrap()["tickets"][0];
    assert_eq!(row["comments"], 3);
    assert!(row["updated"].is_string() && row.get("title").is_none());
    let all = p.json(&["status", "--all"]);
    assert_eq!(all["lists"][3]["tickets"].as_array().unwrap().len(), 3);
    assert!(
        p.run(&["status", "--full", "--json"]).len() > 16_000,
        "--full carries everything"
    );
    assert!(ids(&p.json(&["status", "--exclude-list", "done"])["lists"])
        .iter()
        .all(|l| l != "done"));
    // The text view ends with where to go next.
    assert!(p.run(&["status"]).contains("wipe ticket show <id>"));
}

/// A ticket with a long history prints a bounded view, and says how to see more.
#[test]
fn ticket_show_is_bounded_and_hints_at_the_rest() {
    let p = Project::with_board("Show");
    p.json(&[
        "ticket",
        "create",
        "Busy",
        "--list",
        "todo",
        "--body",
        &"body ".repeat(1000),
    ]);
    let store = wipe_core::Store::open(p.path()).unwrap();
    for i in 0..40 {
        let body = format!("comment {i:02} {}", "word ".repeat(300));
        wipe_core::ops::add_comment(
            &store,
            "T-001",
            "Tester <t@example.com>",
            &body,
            chrono::Utc::now(),
        )
        .unwrap();
    }
    let text = p.run(&["ticket", "show", "T-001"]);
    assert!(
        text.len() < 6_000,
        "default view stays small: {} bytes",
        text.len()
    );
    assert!(
        text.contains("40 comments, newest 3")
            && text.contains("comment 39")
            && !text.contains("comment 36"),
        "{text}"
    );
    let hint = text.lines().last().unwrap();
    assert!(
        hint.starts_with("more:") && hint.contains("--comments all") && hint.contains("--full"),
        "{hint}"
    );

    let v = p.json(&["ticket", "show", "T-001"]);
    assert_eq!(
        (
            v["comments"].as_array().unwrap().len(),
            v["comments_total"].as_u64()
        ),
        (3, Some(40))
    );
    assert_eq!(
        v["body_chars"], 5000,
        "clipped strings say how long they really are"
    );
    assert!(
        v.get("activity").is_none()
            && v.get("version").is_none()
            && v.get("next_comment").is_none()
    );

    let all = p.json(&["ticket", "show", "T-001", "--comments", "all"]);
    assert_eq!(all["comments"].as_array().unwrap().len(), 40);
    assert_eq!(
        p.json(&["ticket", "show", "T-001", "--comments", "1"])["comments"][0]["body"]
            .as_str()
            .unwrap()[..10],
        *"comment 39"
    );
    let full = p.json(&["ticket", "show", "T-001", "--full"]);
    assert_eq!(full["body"].as_str().unwrap().len(), 5000);
    assert!(full["activity"].is_array() && full["comments"][0].get("body_chars").is_none());
    p.json_err(&["ticket", "show", "T-001", "--comments", "lots"]);
}

/// Pasted screenshots and terminal escape sequences never reach an agent's
/// context, in any view.
#[test]
fn encoded_blobs_and_escapes_never_reach_stdout() {
    let p = Project::with_board("Guard");
    let blob = "iVBORw0KGgoAAAANSUhEUgAA".repeat(8_500); // ~200 KB
    std::fs::write(
        p.path().join("shot.md"),
        format!("see ![shot](data:image/png;base64,{blob}) and \u{1b}[2J\u{1b}]0;owned\u{7}done"),
    )
    .unwrap();
    p.json(&[
        "ticket",
        "create",
        "\u{1b}[31mred\u{1b}[0m title",
        "--list",
        "todo",
        "--body-file",
        "shot.md",
    ]);
    p.json_as(
        "bilal",
        &["comment", "add", "T-001", "--body-file", "shot.md"],
    );
    p.json_as(
        "bilal",
        &[
            "forum",
            "post",
            "-t",
            "Screenshot",
            "--body-file",
            "shot.md",
        ],
    );

    for args in [
        vec!["status"],
        vec!["ticket", "list"],
        vec!["ticket", "show", "T-001"],
        vec!["ticket", "show", "T-001", "--full"],
        vec!["comment", "list", "T-001", "--full"],
        vec!["inbox", "--all"],
        vec!["search", "shot"],
        vec!["forum", "show", "F-1", "--full"],
        vec!["forum", "search", "see"],
        vec!["watch", "--timeout", "1s", "--interval", "100"],
    ] {
        for json in [false, true] {
            let mut a = args.clone();
            if json {
                a.push("--json");
            }
            let out = p.cmd(&a).output().unwrap();
            assert!(out.status.success() || a[0] == "watch", "{a:?}");
            let s = String::from_utf8_lossy(&out.stdout);
            assert!(
                !s.contains('\u{1b}') && !s.contains("\\u001b"),
                "{a:?} leaked an escape"
            );
            assert!(
                !s.contains("iVBORw0KGgoAAAANSUhEUgAAiVBOR"),
                "{a:?} leaked the blob"
            );
            assert!(s.len() < 20_000, "{a:?}: {} bytes", s.len());
        }
    }
    assert!(p
        .run(&["ticket", "show", "T-001"])
        .contains("[image/png base64, "));
}

// --- finding things -----------------------------------------------------------------

#[test]
fn search_ranks_explains_and_filters() {
    let p = Project::with_board("Search");
    p.json(&[
        "ticket",
        "create",
        "Foam rework",
        "--list",
        "todo",
        "--label",
        "ocean",
    ]);
    p.json(&[
        "ticket",
        "create",
        "Shore waves",
        "--list",
        "todo",
        "--body",
        "the foam near the shore looks static",
    ]);
    p.json(&["ticket", "create", "Unrelated", "--list", "done"]);
    p.json_as(
        "bob",
        &["comment", "add", "T-003", "Blocked on the FOAM shader"],
    );
    p.json(&[
        "forum",
        "post",
        "-t",
        "Water notes",
        "-b",
        "foam needs a persistence buffer",
    ]);

    let v = p.json(&["search", "foam"]);
    assert_eq!(
        ids(&v["tickets"]),
        ["T-001", "T-002", "T-003"],
        "title hits first"
    );
    assert!(v["tickets"][0].get("matched").is_none());
    assert_eq!(v["tickets"][1]["matched"], "body");
    assert_eq!(
        v["tickets"][1]["snippet"],
        "the foam near the shore looks static"
    );
    assert_eq!(v["tickets"][2]["matched"], "comment c-1 by bob");
    assert_eq!(ids(&v["posts"]), ["F-1"]);
    assert_eq!(v["total"], json!({ "tickets": 3, "posts": 1 }));

    for (q, want) in [
        ("foam label:ocean", vec!["T-001"]),
        ("foam is:open", vec!["T-001", "T-002"]),
        ("foam -static", vec!["T-001", "T-003"]),
        ("\"near the shore\"", vec!["T-002"]),
        ("t2", vec!["T-002"]),
    ] {
        assert_eq!(
            ids(&p.json(&["search", q, "--tickets"])["tickets"]),
            want,
            "{q}"
        );
    }
    let text = p.run(&["search", "foam"]);
    assert!(
        text.contains("body: the foam near") && text.contains("3 tickets, 1 post"),
        "{text}"
    );
    p.json_err(&["search", "x", "is:sleeping"]);
}

#[test]
fn ticket_list_filters_fields_search_and_blockers() {
    let p = Project::with_board("Filters");
    p.json(&["ticket", "create", "a", "--list", "todo", "--label", "app"]);
    p.json(&[
        "ticket",
        "create",
        "b bug",
        "--list",
        "todo",
        "--label",
        "app",
        "--label",
        "bug",
        "--blocked-by",
        "T-001",
    ]);
    p.json(&["ticket", "create", "c", "--list", "done", "--label", "app"]);
    p.json(&[
        "ticket",
        "create",
        "d bug",
        "--list",
        "backlog",
        "--assignee",
        "Tester <t@example.com>",
    ]);

    for (args, want) in [
        (
            vec!["--exclude-list", "done"],
            vec!["T-004", "T-001", "T-002"],
        ),
        (vec!["--label", "app", "--label", "bug"], vec!["T-002"]),
        (vec!["--assignee", "me"], vec!["T-004"]),
        (
            vec!["--list", "todo", "--list", "done"],
            vec!["T-001", "T-002", "T-003"],
        ),
        (vec!["--search", "bug", "--list", "todo"], vec!["T-002"]),
        (vec!["-q", "bug"], vec!["T-004", "T-002"]),
        (vec!["--ready"], vec!["T-004", "T-001"]),
        (vec!["--blocked"], vec!["T-002"]),
        (vec!["--since", "2999-01-01"], vec![]),
    ] {
        let mut a = vec!["ticket", "list"];
        a.extend(&args);
        assert_eq!(ids(&p.json(&a)), want, "{args:?}");
    }
    assert_eq!(
        p.json(&["ticket", "list", "--limit", "2"])
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Default rows: what it is, where, and what it waits on.
    assert_eq!(
        p.json(&["ticket", "list", "--list", "todo"])[1],
        json!({ "id": "T-002", "title": "b bug", "list": "todo", "labels": ["app", "bug"], "blocked_by": ["T-001"] })
    );
    assert_eq!(
        p.json(&["ticket", "list", "--fields", "id,title"])[0],
        json!({ "id": "T-004", "title": "d bug" })
    );
    assert!(
        p.json_err(&["ticket", "list", "--fields", "id,nope"])["error"]
            .as_str()
            .unwrap()
            .contains("available")
    );
    assert!(p.json(&["ticket", "list", "--full", "--limit", "1"])[0]["activity"].is_array());

    // Dependencies: cycles refused; finishing a blocker frees what waits on it.
    let e = p.json_err(&["ticket", "block", "T-001", "--by", "T-002"]);
    assert!(e["error"].as_str().unwrap().contains("cycle"), "{e}");
    p.json(&["ticket", "close", "T-001"]);
    assert!(ids(&p.json(&["ticket", "list", "--blocked"])).is_empty());
    assert_eq!(
        p.json(&["ticket", "unblock", "T-002", "--by", "T-001"])["blocked_by"],
        json!([])
    );
}

// --- edit and review ------------------------------------------------------------------

#[test]
fn one_call_edit_and_original_note() {
    let p = Project::with_board("Edit");
    p.json_as(
        "bilal",
        &[
            "ticket",
            "create",
            "raw note",
            "--list",
            "backlog",
            "--body",
            "buy milk\nand eggs",
        ],
    );
    p.json(&["ticket", "create", "dependency", "--list", "todo"]);
    std::fs::write(p.path().join("plan.md"), "## Plan\n1. milk\n2. eggs").unwrap();

    let r = p.json_as(
        "claude",
        &[
            "ticket",
            "edit",
            "T-001",
            "-t",
            "Shopping",
            "--body-file",
            "plan.md",
            "--label",
            "home",
            "--label",
            "errand",
            "--assignee",
            "claude",
            "--blocked-by",
            "T-002",
            "--to",
            "todo",
            "-m",
            "rewrote the note",
        ],
    );
    let changed: Vec<&str> = r["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    for c in [
        "blocked_by",
        "title",
        "body",
        "labels",
        "assignees",
        "list",
        "comment",
    ] {
        assert!(changed.contains(&c), "{c} missing from {changed:?}");
    }
    assert_eq!(r["list"], "todo");

    let t = p.json(&["ticket", "show", "T-001"]);
    assert_eq!(
        (t["title"].as_str(), t["blocked_by"].clone()),
        (Some("Shopping"), json!(["T-002"]))
    );
    assert!(
        t.get("original").is_none(),
        "the creator's wording is kept, shown with --full"
    );
    let o = &p.json(&["ticket", "show", "T-001", "--full"])["original"];
    assert_eq!(
        (
            o["title"].as_str(),
            o["body"].as_str(),
            o["author"].as_str()
        ),
        (Some("raw note"), Some("buy milk\nand eggs"), Some("bilal"))
    );

    // --remove-label / --unassign / --unblock undo those, still in one call.
    p.json_as(
        "claude",
        &[
            "ticket",
            "edit",
            "T-001",
            "--remove-label",
            "errand",
            "--unassign",
            "claude",
            "--unblock",
            "T-002",
        ],
    );
    let t = p.json(&["ticket", "show", "T-001"]);
    assert_eq!(t["labels"], json!(["home"]));
    assert!(t.get("assignees").is_none() && t.get("blocked_by").is_none());

    // Nothing to change names the options; a bad list writes nothing.
    assert!(p.json_err(&["ticket", "edit", "T-001"])["error"]
        .as_str()
        .unwrap()
        .contains("--to <list>"));
    assert!(
        p.json_err(&["ticket", "edit", "T-001", "-t", "Renamed", "--to", "nowhere"])["error"]
            .as_str()
            .unwrap()
            .contains("nowhere")
    );
    assert_eq!(p.json(&["ticket", "show", "T-001"])["title"], "Shopping");
    let c = p.json(&["ticket", "show", "T-001", "--comments-only"]);
    assert!(c.get("body").is_none() && c["comments"].is_array());
}

#[test]
fn review_loop_submit_reject_approve() {
    let p = Project::with_board("Review");
    p.json_as("bilal", &["ticket", "create", "Feature", "--list", "todo"]);
    let e = p.json_err(&["ticket", "submit", "T-001", "-m", "done"]);
    assert!(
        e["error"]
            .as_str()
            .unwrap()
            .contains("wipe list add Review"),
        "{e}"
    );
    p.json(&["list", "add", "Review"]);

    let r = p.json_as(
        "claude",
        &[
            "ticket",
            "submit",
            "T-001",
            "-m",
            "implemented",
            "--tested",
            "unit tests",
            "--untested",
            "iOS",
        ],
    );
    assert_eq!(
        (r["list"].as_str(), r["comment"].as_str()),
        (Some("review"), Some("c-1"))
    );
    let c = p.json(&["comment", "list", "T-001"])["comments"][0]["body"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        c.contains("implemented") && c.contains("**Tested**") && c.contains("- iOS"),
        "{c}"
    );
    // The reviewer (who filed it) hears about it...
    let events = |who: &str| -> Vec<Value> {
        p.json_as(who, &["inbox"])["items"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|i| i["events"].as_array().unwrap().clone())
            .collect()
    };
    assert!(events("bilal")
        .iter()
        .any(|e| e["event"] == "commented" && e["by"] == "claude"));

    // ...a rejection needs a reason, goes back to rework, and reaches the submitter.
    assert!(p.json_err(&["ticket", "reject", "T-001"])["error"]
        .as_str()
        .unwrap()
        .contains("reason"));
    assert_eq!(
        p.json_as(
            "bilal",
            &["ticket", "reject", "T-001", "-m", "crashes on launch"]
        )["list"],
        "todo"
    );
    assert!(events("claude")
        .iter()
        .any(|e| e["detail"].as_str().unwrap_or("").contains("crashes")));

    // Approval respects acceptance criteria.
    p.json(&["criteria", "add", "T-001", "--text", "no crash"]);
    p.json_as("claude", &["ticket", "submit", "T-001", "-m", "fixed"]);
    assert!(p.json_err(&["ticket", "approve", "T-001"])["error"]
        .as_str()
        .unwrap()
        .contains("ac-1: no crash"));
    p.json(&["criteria", "check", "T-001", "ac-1"]);
    assert_eq!(
        p.json_as("bilal", &["ticket", "approve", "T-001", "-m", "ship it"])["list"],
        "done"
    );
    // A configured rework list wins over detection.
    p.json(&["config", "set", "board.rework_list", "backlog"]);
    p.json_as("claude", &["ticket", "submit", "T-001", "-m", "again"]);
    assert_eq!(
        p.json_as("bilal", &["ticket", "reject", "T-001", "-m", "no"])["list"],
        "backlog"
    );
}

#[test]
fn forum_pin_and_bounded_digest() {
    let p = Project::with_board("Digest");
    p.json(&[
        "forum",
        "post",
        "-t",
        "Build rule",
        "-b",
        "Always run flutter clean first.",
    ]);
    p.json(&["forum", "reply", "F-1", "-b", "And bump the build number."]);
    p.json(&["forum", "post", "-t", "Chatter", "-b", "not durable"]);
    assert!(p.run(&["forum", "digest"]).contains("wipe forum pin"));

    p.json(&["forum", "pin", "F-1"]);
    assert_eq!(p.json(&["forum", "pin", "F-1"])["changed"], false);
    assert!(p.json_err(&["forum", "pin", "F-1.1"])["error"]
        .as_str()
        .unwrap()
        .contains("whole threads"));
    let d = p.run(&["forum", "digest"]);
    assert!(
        d.contains("## F-1 - Build rule")
            && d.contains("flutter clean")
            && d.contains("F-1.1")
            && !d.contains("Chatter")
    );

    // Size-bounded, however long the pinned threads are.
    p.json(&["forum", "post", "-t", "Huge", "-b", &"word ".repeat(3000)]);
    p.json(&["forum", "pin", "F-3"]);
    let d = p.json(&["forum", "digest", "--max-bytes", "1500"]);
    assert!(d["bytes"].as_u64().unwrap() <= 1500, "{}", d["bytes"]);
    assert!(d["digest"]
        .as_str()
        .unwrap()
        .contains("wipe forum show F-3"));
    p.json(&["forum", "unpin", "F-3"]);
    assert!(!p.run(&["forum", "digest"]).contains("Huge"));
}

// --- watch --------------------------------------------------------------------------

/// Spawn `wipe watch` as `who` and return once it has taken its snapshot.
fn watch(p: &Project, who: &str, args: &[&str]) -> Child {
    let mut a = vec!["watch", "--interval", "100"];
    a.extend(args);
    let mut c = p.cmd(&a);
    c.env("WIPE_AUTHOR", who)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stderr.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.starts_with("watching"), "{line}");
    child
}

/// Give freshly spawned watches time to take their snapshot (it follows the banner).
fn settle() {
    std::thread::sleep(Duration::from_millis(300));
}

#[test]
fn watch_reports_others_changes_in_one_line() {
    let p = Project::with_board("Watch");
    p.json(&["ticket", "create", "Fix the foam", "--list", "todo"]);
    p.json(&["ticket", "create", "Quiet", "--list", "todo"]);

    // Nothing happens to T-002 within the timeout: exit 124, nothing on stdout.
    let quiet = watch(&p, "claude", &["T-002", "--timeout", "1s"]);
    // --until: the first matching change by someone else ends the watch.
    let w = watch(&p, "claude", &["T-001", "--until", "--timeout", "20s"]);
    let stream = watch(
        &p,
        "claude",
        &[
            "--json",
            "in-progress",
            "--on",
            "commented",
            "--count",
            "1",
            "--timeout",
            "20s",
        ],
    );
    settle();
    p.json_as("claude", &["comment", "add", "T-001", "my own note"]); // never reported
    settle();
    p.json_as("bilal", &["ticket", "move", "T-001", "in-progress"]);
    p.json_as(
        "bilal",
        &["comment", "add", "T-001", "please re-check\nthe edges"],
    );

    let out = w.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "T-001 moved todo → in-progress · bilal · Fix the foam\n"
    );
    let out = stream.wait_with_output().unwrap();
    assert!(out.status.success());
    let ev: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        (
            ev["event"].as_str(),
            ev["id"].as_str(),
            ev["by"].as_str(),
            ev["list"].as_str(),
            ev["detail"].as_str()
        ),
        (
            Some("commented"),
            Some("T-001"),
            Some("bilal"),
            Some("in-progress"),
            Some("please re-check the edges")
        )
    );

    let out = quiet.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(124));
    assert!(out.stdout.is_empty());
    assert!(p.json_err(&["watch", "nowhere"])["error"]
        .as_str()
        .unwrap()
        .contains("in-progress"));
}

// --- ticket ids ---------------------------------------------------------------------

#[test]
fn hex_ids_any_spelling_translation_and_repair() {
    // New boards: fixed-width hex ids, accepted in any common spelling.
    let p = Project::with_board("Hex");
    for i in 0..11 {
        p.json(&["ticket", "create", &format!("t{i}"), "--list", "todo"]);
    }
    for typed in ["T-00B", "t00b", "T-B", "tb", "T b"] {
        assert_eq!(p.json(&["ticket", "show", typed])["id"], "T-00B", "{typed}");
    }
    assert_eq!(
        p.json(&["ticket", "move", "t3", "done"])["id"],
        "T-003",
        "receipts are canonical"
    );

    // A board from before 0.4.1 (no `ids` field) keeps decimal ids...
    let old = Project::with_board("Legacy");
    let bj = old.path().join(".wipe/board.json");
    let mut b: Value = serde_json::from_str(&std::fs::read_to_string(&bj).unwrap()).unwrap();
    b.as_object_mut().unwrap().remove("ids");
    std::fs::write(&bj, serde_json::to_string_pretty(&b).unwrap()).unwrap();
    for i in 0..12 {
        old.json(&["ticket", "create", &format!("old {i}"), "--list", "todo"]);
    }
    assert_eq!(old.json(&["ticket", "show", "T-12"])["id"], "T-12");

    // ...and is offered the translation once (a stderr notice, never a prompt).
    std::fs::remove_file(old.path().join(".wipe/.cache/ids-translation-offered")).ok();
    let stderr = |args: &[&str]| {
        String::from_utf8_lossy(&old.cmd(args).output().unwrap().stderr).to_string()
    };
    assert!(stderr(&["status", "--json"]).contains("translate-ids"));
    assert!(!stderr(&["status", "--json"]).contains("translate-ids"));
    assert!(old.json_err(&["board", "translate-ids"])["error"]
        .as_str()
        .unwrap()
        .contains("--yes"));

    old.json(&["comment", "add", "T-3", "blocked by T-12, see T-10"]);
    old.json(&[
        "forum",
        "post",
        "-t",
        "about T-12",
        "-b",
        "T-12 is done",
        "--ref",
        "t12",
    ]);
    old.json(&["ticket", "delete", "T-5", "--yes"]);
    // Snapshot the forum and trash to replay a hand-repaired 0.4.1 board below.
    let dirs = [
        old.path().join(".wipe/forum"),
        old.path().join(".wipe/.cache/trash"),
    ];
    let saved: Vec<Vec<(std::path::PathBuf, Vec<u8>)>> = dirs
        .iter()
        .map(|d| {
            std::fs::read_dir(d)
                .unwrap()
                .flatten()
                .map(|e| (e.path(), std::fs::read(e.path()).unwrap()))
                .collect()
        })
        .collect();

    assert_eq!(
        old.json(&["board", "translate-ids", "--yes"])["translated"],
        12
    );
    let t = old.json(&["ticket", "show", "T-12"]);
    assert_eq!(
        (t["id"].as_str(), t["legacy_id"].as_str()),
        (Some("T-00C"), Some("T-12")),
        "old ids still resolve"
    );
    assert_eq!(
        old.json(&["comment", "list", "T-3"])["comments"][0]["body"],
        "blocked by T-00C, see T-00A"
    );
    assert_eq!(
        old.json(&["forum", "show", "F-1"])["post"]["refs"],
        json!(["T-00C"])
    );
    assert_eq!(
        old.json(&["ticket", "create", "next", "--list", "todo"])["id"],
        "T-00D"
    );

    // A board that went hex with its forum and trash left decimal is finished by
    // the next command, once.
    for (d, files) in dirs.iter().zip(&saved) {
        std::fs::remove_dir_all(d).unwrap();
        std::fs::create_dir_all(d).unwrap();
        for (path, bytes) in files {
            std::fs::write(path, bytes).unwrap();
        }
    }
    std::fs::remove_file(old.path().join(".wipe/.cache/ids-repaired-1")).ok();
    assert!(stderr(&["status", "--json"]).contains("leftover decimal"));
    assert!(
        !stderr(&["status", "--json"]).contains("leftover decimal"),
        "only once"
    );
    assert_eq!(
        old.json(&["forum", "show", "F-1"])["post"]["refs"],
        json!(["T-00C"])
    );
    assert_eq!(ids(&old.json(&["trash", "list"])["trash"]), ["T-005"]);
    assert_eq!(
        old.json(&["board", "translate-ids", "--yes"])["translated"],
        0
    );
}

// --- awareness between commands ---------------------------------------------------

/// Run as `who`, returning (stdout, stderr).
fn run_as(p: &Project, who: &str, args: &[&str]) -> (String, String) {
    let mut c = p.cmd(args);
    c.env("WIPE_AUTHOR", who);
    let out = c.output().unwrap();
    assert!(out.status.success(), "{args:?} as {who} failed");
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn any_command_reports_what_others_changed_once() {
    let p = Project::with_board("Aware");
    let mine = p.json_as(
        "ada",
        &[
            "ticket",
            "create",
            "Mine",
            "--list",
            "todo",
            "--assignee",
            "bot",
        ],
    );
    let mine = mine["id"].as_str().unwrap().to_string();
    // The first command only records where bot's awareness starts.
    let (_, err) = run_as(&p, "bot", &["ticket", "list"]);
    assert!(!err.contains("[wipe]"), "{err}");

    let bug = p.json_as(
        "ada",
        &["ticket", "create", "Login crash", "--list", "todo"],
    );
    p.json_as(
        "ada",
        &["comment", "add", &mine, "please also cover the edge case"],
    );

    // --json stdout stays one object; the updates go to stderr.
    let (out, err) = run_as(&p, "bot", &["label", "list", "--json"]);
    serde_json::from_str::<Value>(&out).expect("stdout is still one JSON object");
    assert!(err.contains("[wipe] 2 board updates"), "{err}");
    assert!(
        err.contains(&format!(
            "new ticket in Todo by ada: {}",
            bug["id"].as_str().unwrap()
        )),
        "{err}"
    );
    assert!(
        err.contains(&format!("ada commented on your ticket {mine}")),
        "{err}"
    );

    // Reported once, and bot's own changes are never news to it.
    p.json_as("bot", &["ticket", "move", &mine, "in-progress"]);
    let (_, err) = run_as(&p, "bot", &["ticket", "list"]);
    assert!(!err.contains("[wipe]"), "{err}");

    // `status` shows them inline instead.
    p.json_as("ada", &["comment", "add", &mine, "@bot ping"]);
    let (out, err) = run_as(&p, "bot", &["status"]);
    assert!(out.contains("Since your last wipe command:"), "{out}");
    assert!(out.contains("ada mentioned you on your ticket"), "{out}");
    assert!(!err.contains("[wipe]"), "{err}");
}

#[test]
fn finishing_a_ticket_names_the_work_still_assigned() {
    let p = Project::with_board("Next");
    let a = p.json_as(
        "ada",
        &[
            "ticket",
            "create",
            "First",
            "--list",
            "todo",
            "--assignee",
            "bot",
        ],
    );
    let b = p.json_as(
        "ada",
        &[
            "ticket",
            "create",
            "Second",
            "--list",
            "backlog",
            "--assignee",
            "bot",
        ],
    );
    p.json_as("ada", &["ticket", "create", "Not mine", "--list", "todo"]);
    let (a, b) = (a["id"].as_str().unwrap(), b["id"].as_str().unwrap());

    // A move that doesn't finish anything stays quiet.
    let (_, err) = run_as(&p, "bot", &["ticket", "move", a, "in-progress"]);
    assert!(!err.contains("still assigned"), "{err}");

    let (_, err) = run_as(&p, "bot", &["ticket", "close", a]);
    assert!(
        err.contains(&format!("still assigned to you: {b} \"Second\" (Backlog)")),
        "{err}"
    );
    assert!(!err.contains("Not mine"), "{err}");

    // Nothing left: nothing said.
    let (_, err) = run_as(&p, "bot", &["ticket", "close", b]);
    assert!(!err.contains("still assigned"), "{err}");
}
