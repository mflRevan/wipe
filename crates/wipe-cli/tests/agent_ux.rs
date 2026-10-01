//! 0.4 agent-experience contract: text input from files/stdin, compact output,
//! write receipts, the review loop, dependencies, the forum digest, the forced
//! identity, and the commit hint. Each test drives the real binary.

mod common;

use common::Project;
use serde_json::Value;

fn lines_of(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect()
}

// --- text input ----------------------------------------------------------------

#[test]
fn multiline_bodies_survive_via_file_and_stdin() {
    let p = Project::with_board("Text");
    let body = "line one\n\nline three with \"quotes\" and $dollars\n- a list";

    // --body-file: a BOM and trailing newlines (editor/PowerShell artifacts) are
    // not content; everything else is kept byte for byte.
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

    // --body - reads stdin.
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

    // Comments: positional, file, and stdin forms all work and agree.
    p.json(&["comment", "add", "T-001", "positional comment"]);
    std::fs::write(p.path().join("c.md"), "multi\nline\ncomment").unwrap();
    p.json(&["comment", "add", "T-001", "--body-file", "c.md"]);
    let out = p.with_stdin(
        &["comment", "add", "T-001", "-b", "-", "--json"],
        "from\nstdin",
    );
    assert!(out.status.success());
    let comments = p.json(&["comment", "list", "T-001"])["comments"].clone();
    let bodies: Vec<&str> = comments
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["body"].as_str().unwrap())
        .collect();
    assert_eq!(
        bodies,
        vec!["positional comment", "multi\nline\ncomment", "from\nstdin"]
    );

    // Forum post + reply + edit take files too.
    p.json(&["forum", "post", "-t", "Notes", "--body-file", "c.md"]);
    p.json(&["forum", "reply", "F-1", "--body-file", "b.md"]);
    p.json(&["forum", "edit", "F-1", "--body-file", "b.md"]);
    let t = p.json(&["forum", "show", "F-1"]);
    assert_eq!(t["post"]["body"], body);
    assert_eq!(t["post"]["replies"][0]["body"], body);

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

#[test]
fn positional_title_and_multi_label_forms() {
    let p = Project::with_board("Forms");
    let r = p.json(&["ticket", "create", "Positional title", "--list", "todo"]);
    assert_eq!(r["title"], "Positional title");
    // Both forms at once is ambiguous and refused by the parser.
    let out = p
        .cmd(&["ticket", "create", "A", "--title", "B", "--list", "todo"])
        .output()
        .unwrap();
    assert!(!out.status.success());

    let r = p.json(&["label", "assign", "T-001", "app", "bug", "ios"]);
    assert_eq!(r["labels"], serde_json::json!(["app", "bug", "ios"]));
    let r = p.json(&["label", "remove", "T-001", "bug", "ios"]);
    assert_eq!(r["labels"], serde_json::json!(["app"]));
}

// --- output shape ----------------------------------------------------------------

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
    assert_eq!(v["ok"], true);
    assert_eq!(v["comment"], "c-6");
    assert!(receipt.len() < 200, "receipt too large: {receipt}");
    // Compact JSON: one line.
    assert_eq!(receipt.trim().lines().count(), 1);

    // --echo returns the whole ticket.
    let full = p.json(&["--echo", "comment", "add", "T-001", "echoed"]);
    assert_eq!(full["comments"].as_array().unwrap().len(), 7);
    assert!(full["activity"].is_array());

    // Edits, moves, labels also answer with receipts.
    let v = p.json(&["ticket", "move", "T-001", "--to", "done"]);
    assert_eq!(
        v,
        serde_json::json!({ "ok": true, "id": "T-001", "list": "done" })
    );

    // --pretty opts back into indented JSON.
    let pretty = p.run(&["ticket", "show", "T-001", "--json", "--pretty"]);
    assert!(pretty.lines().count() > 10);
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
        raw.len() < 1500,
        "compact status must not carry bodies/comments ({} bytes)",
        raw.len()
    );
    let s: Value = serde_json::from_str(&raw).unwrap();
    let lists = s["lists"].as_array().unwrap();
    let todo = lists.iter().find(|l| l["id"] == "todo").unwrap();
    assert_eq!(todo["count"], 1);
    let row = &todo["tickets"][0];
    assert_eq!(row["id"], "T-001");
    assert_eq!(row["comments"], 3);
    assert_eq!(row["labels"], serde_json::json!(["app"]));
    assert!(row.get("body").is_none() && row.get("list").is_none());
    let done = lists.iter().find(|l| l["id"] == "done").unwrap();
    assert_eq!(
        (done["count"].as_u64(), done["collapsed"].as_bool()),
        (Some(3), Some(true))
    );
    assert!(done.get("tickets").is_none());
    assert_eq!(s["identity"], "Tester <t@example.com>");

    // --all lists done tickets; --full restores the whole-content dump.
    let all = p.json(&["status", "--all"]);
    let done = all["lists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == "done")
        .unwrap()
        .clone();
    assert_eq!(done["tickets"].as_array().unwrap().len(), 3);
    let full = p.run(&["status", "--full", "--json"]);
    assert!(full.len() > 16_000, "--full carries everything");
    let ex = p.json(&["status", "--exclude-list", "done"]);
    assert!(ex["lists"]
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["id"] != "done"));

    // Human status ends with the compact-listing pointer.
    let human = p.run(&["status"]);
    assert!(human.contains("wipe ticket list --list"), "{human}");
}

#[test]
fn ticket_list_filters_and_fields() {
    let p = Project::with_board("Filters");
    p.json(&["ticket", "create", "a", "--list", "todo", "--label", "app"]);
    p.json(&[
        "ticket", "create", "b", "--list", "todo", "--label", "app", "--label", "bug",
    ]);
    p.json(&["ticket", "create", "c", "--list", "done", "--label", "app"]);
    p.json(&[
        "ticket",
        "create",
        "d",
        "--list",
        "backlog",
        "--assignee",
        "Tester <t@example.com>",
    ]);

    assert_eq!(
        lines_of(&p.json(&["ticket", "list", "--exclude-list", "done"])),
        ["T-004", "T-001", "T-002"]
    );
    assert_eq!(
        lines_of(&p.json(&["ticket", "list", "--label", "app", "--label", "bug"])),
        ["T-002"]
    );
    assert_eq!(
        lines_of(&p.json(&["ticket", "list", "--assignee", "me"])),
        ["T-004"]
    );
    assert_eq!(
        lines_of(&p.json(&["ticket", "list", "--list", "todo", "--list", "done"])),
        ["T-001", "T-002", "T-003"]
    );
    assert_eq!(
        p.json(&["ticket", "list", "--limit", "2"])
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        p.json(&["ticket", "list", "--since", "1h"])
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(p
        .json(&["ticket", "list", "--since", "2999-01-01"])
        .as_array()
        .unwrap()
        .is_empty());

    let rows = p.json(&["ticket", "list", "--fields", "id,title"]);
    assert_eq!(rows[0], serde_json::json!({ "id": "T-004", "title": "d" }));
    let e = p.json_err(&["ticket", "list", "--fields", "id,nope"]);
    assert!(e["error"].as_str().unwrap().contains("available"), "{e}");
    let e = p.json_err(&["ticket", "list", "--list", "nosuch"]);
    assert!(
        e["error"].as_str().unwrap().contains("todo"),
        "lists the real lists: {e}"
    );

    let full = p.json(&["ticket", "list", "--full", "--limit", "1"]);
    assert!(full[0]["activity"].is_array());
}

// --- one-call edit, original note, show --------------------------------------------

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
    assert_eq!(t["title"], "Shopping");
    assert_eq!(t["original"]["title"], "raw note");
    assert_eq!(t["original"]["body"], "buy milk\nand eggs");
    assert_eq!(t["original"]["author"], "bilal");
    assert_eq!(t["original"]["rewritten_by"], "claude");
    assert_eq!(t["relations"][0]["target"], "T-002");

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
    assert_eq!(t["labels"], serde_json::json!(["home"]));
    assert!(t.get("assignees").is_none() && t.get("relations").is_none());

    // Nothing to change is an error that names the options.
    let e = p.json_err(&["ticket", "edit", "T-001"]);
    assert!(e["error"].as_str().unwrap().contains("--to <list>"), "{e}");
    // A bad list is rejected before anything is written.
    let before = p.json(&["ticket", "show", "T-001"]);
    let e = p.json_err(&[
        "ticket", "edit", "T-001", "-t", "Renamed", "--to", "nowhere",
    ]);
    assert!(e["error"].as_str().unwrap().contains("nowhere"));
    assert_eq!(
        p.json(&["ticket", "show", "T-001"])["title"],
        before["title"]
    );

    // show --comments-only / --no-activity trim the payload.
    let c = p.json(&["ticket", "show", "T-001", "--comments-only"]);
    assert!(c.get("body").is_none() && c["comments"].is_array());
    assert!(p
        .json(&["ticket", "show", "T-001", "--no-activity"])
        .get("activity")
        .is_none());
}

#[test]
fn ticket_show_lists_commits_that_mention_it() {
    let p = Project::with_board("Commits");
    p.git_init();
    p.json(&["ticket", "create", "Login", "--list", "todo"]);
    for (f, msg) in [
        ("a", "fix login redirect (T-001)"),
        ("b", "unrelated T-0011 work"),
        ("c", "polish\n\nrefs T-001"),
    ] {
        std::fs::write(p.path().join(f), f).unwrap();
        p.git(&["add", f]);
        p.git(&["commit", "-q", "-m", msg]);
    }
    let t = p.json(&["ticket", "show", "T-001"]);
    let subjects: Vec<&str> = t["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects, vec!["polish", "fix login redirect (T-001)"]);
    assert!(p
        .json(&["ticket", "show", "T-001", "--no-commits"])
        .get("commits")
        .is_none());
}

// --- review loop + dependencies ----------------------------------------------------

#[test]
fn review_loop_submit_reject_approve() {
    let p = Project::with_board("Review");
    p.json_as("bilal", &["ticket", "create", "Feature", "--list", "todo"]);

    // No review list yet: a helpful refusal.
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
    let c = &p.json(&["comment", "list", "T-001"])["comments"][0]["body"];
    let c = c.as_str().unwrap();
    assert!(
        c.contains("implemented") && c.contains("**Tested**") && c.contains("- iOS"),
        "{c}"
    );

    // The reviewer sees it in their inbox (they authored the ticket).
    let inbox = p.json_as("bilal", &["inbox"]);
    assert!(inbox["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "comment"));

    // Reject needs a reason; it goes back to the rework list with it.
    let e = p.json_err(&["ticket", "reject", "T-001"]);
    assert!(e["error"].as_str().unwrap().contains("reason"), "{e}");
    let r = p.json_as(
        "bilal",
        &["ticket", "reject", "T-001", "-m", "crashes on launch"],
    );
    assert_eq!(r["list"], "todo");
    // ...and the submitter hears about it.
    let inbox = p.json_as("claude", &["inbox"]);
    assert!(inbox["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["detail"].as_str().unwrap_or("").contains("crashes")));

    // Approval respects acceptance criteria.
    p.json(&["criteria", "add", "T-001", "--text", "no crash"]);
    p.json_as("claude", &["ticket", "submit", "T-001", "-m", "fixed"]);
    let e = p.json_err(&["ticket", "approve", "T-001"]);
    assert!(
        e["error"].as_str().unwrap().contains("ac-1: no crash"),
        "{e}"
    );
    p.json(&["criteria", "check", "T-001", "ac-1"]);
    let r = p.json_as("bilal", &["ticket", "approve", "T-001", "-m", "ship it"]);
    assert_eq!(r["list"], "done");

    // Configured workflow lists win over detection.
    p.json(&["config", "set", "board.rework_list", "backlog"]);
    assert_eq!(
        p.json(&["config", "get", "board.rework_list"])["value"],
        "backlog"
    );
    let e = p.json_err(&["config", "set", "board.review_list", "nope"]);
    assert!(e["error"].as_str().unwrap().contains("no list"), "{e}");
}

#[test]
fn blockers_and_ready_listing() {
    let p = Project::with_board("Deps");
    p.json(&["ticket", "create", "TestFlight build", "--list", "todo"]);
    p.json(&[
        "ticket",
        "create",
        "Purchase tests",
        "--list",
        "todo",
        "--blocked-by",
        "T-001",
    ]);
    p.json(&["ticket", "create", "Release notes", "--list", "todo"]);
    p.json(&["ticket", "block", "T-003", "--by", "T-002"]);

    assert_eq!(lines_of(&p.json(&["ticket", "list", "--ready"])), ["T-001"]);
    assert_eq!(
        lines_of(&p.json(&["ticket", "list", "--blocked"])),
        ["T-002", "T-003"]
    );
    let e = p.json_err(&["ticket", "block", "T-001", "--by", "T-003"]);
    assert!(e["error"].as_str().unwrap().contains("cycle"), "{e}");

    // Finishing a blocker frees what waits on it.
    p.json(&["ticket", "close", "T-001"]);
    assert_eq!(lines_of(&p.json(&["ticket", "list", "--ready"])), ["T-002"]);
    let row = p.json(&["ticket", "list", "--list", "todo"]);
    assert_eq!(row[1]["blocked_by"], serde_json::json!(["T-002"]));
    let r = p.json(&["ticket", "unblock", "T-003", "--by", "T-002"]);
    assert_eq!(r["blocked_by"], serde_json::json!([]));
}

// --- forum digest, inbox --all ---------------------------------------------------

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
    let empty = p.run(&["forum", "digest"]);
    assert!(empty.contains("wipe forum pin"), "{empty}");

    p.json(&["forum", "pin", "F-1"]);
    assert_eq!(p.json(&["forum", "pin", "F-1"])["changed"], false);
    let e = p.json_err(&["forum", "pin", "F-1.1"]);
    assert!(e["error"].as_str().unwrap().contains("whole threads"));

    let d = p.run(&["forum", "digest"]);
    assert!(
        d.contains("## F-1 - Build rule") && d.contains("flutter clean") && d.contains("F-1.1")
    );
    assert!(!d.contains("Chatter"));

    // Size-bounded, however long the pinned threads are.
    let huge = "word ".repeat(3000);
    p.json(&["forum", "post", "-t", "Huge", "-b", &huge]);
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

#[test]
fn inbox_all_shows_everything_others_changed() {
    let p = Project::with_board("Inbox");
    p.json_as("bilal", &["ticket", "create", "Theirs", "--list", "todo"]);
    p.json_as("bilal", &["comment", "add", "T-001", "hello"]);
    // Not assigned/authored/subscribed: the default inbox is empty...
    assert_eq!(p.json_as("claude", &["inbox"])["count"], 0);
    // ...the board-wide view is not, and says why each event is there.
    let all = p.json_as("claude", &["inbox", "--all", "--limit", "1"]);
    assert_eq!(all["count"], 1);
    assert!(all["total"].as_u64().unwrap() >= 2);
    assert_eq!(all["events"][0]["reason"], "board");
}

// --- identity ---------------------------------------------------------------------

#[test]
fn agent_sessions_never_inherit_the_terminal_identity() {
    let p = Project::with_board("Sessions");
    let with = |envs: &[(&str, &str)], args: &[&str]| {
        let mut c = p.bare(args);
        c.env_remove("WIPE_SESSION");
        for (k, v) in envs {
            c.env(k, v);
        }
        c.output().unwrap()
    };
    // The human binds an identity in their terminal tab.
    assert!(with(
        &[("WT_SESSION", "tab-1")],
        &["identity", "use", "bilal@x.com", "--human"]
    )
    .status
    .success());
    assert!(with(
        &[("WT_SESSION", "tab-1")],
        &["ticket", "create", "h", "--list", "todo"]
    )
    .status
    .success());

    // An agent launched from that same tab (inherits WT_SESSION) has its own
    // session and so starts with NO identity.
    let agent = [
        ("WT_SESSION", "tab-1"),
        ("CLAUDE_CODE_SESSION_ID", "abc"),
        ("CLAUDECODE", "1"),
    ];
    let out = with(
        &agent,
        &["ticket", "create", "a", "--list", "todo", "--json"],
    );
    assert!(!out.status.success(), "agent must not write as the human");
    assert!(with(&agent, &["identity", "use", "claude", "--agent"])
        .status
        .success());
    let out = with(
        &agent,
        &[
            "--echo", "ticket", "create", "a", "--list", "todo", "--json",
        ],
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["activity"][0]["actor"], "claude");
    // ...and the human's tab still writes as the human.
    let out = with(
        &[("WT_SESSION", "tab-1")],
        &[
            "--echo", "ticket", "create", "h2", "--list", "todo", "--json",
        ],
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["activity"][0]["actor"], "bilal@x.com");

    // An agent harness with no session id of its own cannot bind one - it is
    // told to use $WIPE_AGENT instead of silently sharing a key.
    let out = with(
        &[("WT_SESSION", "tab-1"), ("AI_AGENT", "other")],
        &["identity", "use", "x", "--json"],
    );
    assert!(!out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["error"].as_str().unwrap().contains("WIPE_AGENT"), "{v}");
}

// --- git story ---------------------------------------------------------------------

#[test]
fn commit_hint_until_first_commit_and_doctor_warnings() {
    let p = Project::with_board("Hint");
    p.git_init();
    let out = p
        .cmd(&["ticket", "create", "A", "--list", "todo", "--json"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("wipe commit"),
        "hint expected on stderr: {err}"
    );
    // The hint is stderr-only: stdout stays one JSON object.
    let _: Value = serde_json::from_slice(&out.stdout).unwrap();

    let d = p.json(&["doctor"]);
    assert!(d["uncommitted_board_files"].as_u64().unwrap() > 0);
    assert!(d["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("wipe commit")));

    p.json(&["commit"]);
    let out = p
        .cmd(&["ticket", "create", "B", "--list", "todo", "--json"])
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("wipe commit"),
        "hint retired after first commit"
    );

    // The npm-shim marker makes doctor explain the Windows truncation.
    let mut c = p.cmd(&["doctor", "--json"]);
    c.env("WIPE_NPM_SHIM", "1");
    let v: Value = serde_json::from_slice(&c.output().unwrap().stdout).unwrap();
    assert_eq!(v["npm_shim"], true);
    if cfg!(windows) {
        assert!(v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("--body-file")));
    }
}

#[test]
fn serve_exposure_config_values() {
    let p = Project::with_board("Expose");
    for (v, want) in [
        ("lan", "lan"),
        ("local", "local"),
        ("localhost", "local"),
        ("tailscale", "tailscale"),
    ] {
        p.json(&["config", "set", "daemon.expose", v]);
        assert_eq!(p.json(&["config", "get", "daemon.expose"])["value"], want);
    }
    let e = p.json_err(&["config", "set", "daemon.expose", "none"]);
    assert!(e["error"].as_str().unwrap().contains("local"), "{e}");
}

// --- ticket id format ----------------------------------------------------------

#[test]
fn hex_ids_any_spelling_and_legacy_translation() {
    // New boards: fixed-width hex ids, accepted in any common spelling.
    let p = Project::with_board("Hex");
    for i in 0..11 {
        p.json(&["ticket", "create", &format!("t{i}"), "--list", "todo"]);
    }
    for typed in ["T-00B", "t00b", "T-B", "tb", "T b"] {
        assert_eq!(p.json(&["ticket", "show", typed])["id"], "T-00B", "{typed}");
    }
    // Receipts print the canonical id whatever was typed.
    assert_eq!(
        p.json(&["ticket", "move", "t3", "--to", "done"])["id"],
        "T-003"
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

    // ...and is offered the translation once (a stderr notice for agents, never a
    // blocking prompt), then never again.
    std::fs::remove_file(old.path().join(".wipe/.cache/ids-translation-offered")).ok();
    let first = old.cmd(&["status", "--json"]).output().unwrap();
    assert!(String::from_utf8_lossy(&first.stderr).contains("translate-ids"));
    let second = old.cmd(&["status", "--json"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&second.stderr).contains("translate-ids"));

    // Off a terminal the translation needs --yes.
    let e = old.json_err(&["board", "translate-ids"]);
    assert!(e["error"].as_str().unwrap().contains("--yes"), "{e}");
    old.json(&["comment", "add", "T-3", "blocked by T-12, see T-10"]);
    let r = old.json(&["board", "translate-ids", "--yes"]);
    assert_eq!(r["translated"], 12);

    // Old ids still resolve; text references were rewritten.
    let t = old.json(&["ticket", "show", "T-12"]);
    assert_eq!(
        (t["id"].as_str(), t["legacy_id"].as_str()),
        (Some("T-00C"), Some("T-12"))
    );
    let c = old.json(&["comment", "list", "T-3"]);
    assert_eq!(c["comments"][0]["body"], "blocked by T-00C, see T-00A");
    assert_eq!(
        old.json(&["ticket", "create", "next", "--list", "todo"])["id"],
        "T-00D"
    );
    assert_eq!(
        old.json(&["board", "translate-ids", "--yes"])["translated"],
        0
    );
}
