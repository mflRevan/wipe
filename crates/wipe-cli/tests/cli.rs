//! End-to-end CLI contract: the real `wipe` binary against throwaway boards -
//! setup, the everyday agent loop, identity gating, git integration, trash, the
//! inbox, and multi-agent collaboration. Each test is one realistic scenario.

mod common;

use std::process::Command as StdCommand;

use assert_cmd::prelude::*;
use common::Project;
use serde_json::{json, Value};

fn ids(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn init_starters_global_defaults_and_discovery() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Demo"]);
    let status = p.json(&["status"]);
    assert_eq!(status["board"], "Demo");
    assert_eq!(
        ids(&status["lists"]),
        ["backlog", "todo", "in-progress", "done"]
    );

    // Commands find the board from any subdirectory, or from anywhere via -C.
    let sub = p.path().join("src").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    let mut c = StdCommand::cargo_bin("wipe").unwrap();
    c.current_dir(&sub)
        .env("WIPE_CONFIG_DIR", p.path())
        .args(["status", "--json"]);
    let v: Value = serde_json::from_slice(&c.output().unwrap().stdout).unwrap();
    assert_eq!(v["board"], "Demo");
    let elsewhere = tempfile::tempdir().unwrap();
    let mut c = StdCommand::cargo_bin("wipe").unwrap();
    c.current_dir(elsewhere.path())
        .env("WIPE_CONFIG_DIR", p.path());
    c.args(["-C", p.path().to_str().unwrap(), "status", "--json"]);
    assert!(c.output().unwrap().status.success());

    // Stored global defaults drive a non-interactive init; `empty` has no lists.
    let g = Project::new();
    g.json(&["config", "--global", "set", "autoserve", "true"]);
    g.json(&["config", "--global", "set", "starter", "empty"]);
    assert_eq!(
        g.json(&["config", "--global", "get", "starter"])["value"],
        "empty"
    );
    let v = g.json(&["init", ".", "--name", "G", "--yes"]);
    assert_eq!(
        (v["starter"].as_str(), v["autoserve"].as_bool()),
        (Some("empty"), Some(true))
    );
    assert!(g.json(&["status"])["lists"].as_array().unwrap().is_empty());
}

#[test]
fn agent_work_loop() {
    let p = Project::with_board("Flow");
    let t = p.json(&[
        "ticket",
        "create",
        "Add login",
        "--list",
        "todo",
        "--priority",
        "high",
    ]);
    assert_eq!(
        (t["id"].as_str(), t["title"].as_str()),
        (Some("T-001"), Some("Add login"))
    );
    p.json(&[
        "ticket",
        "create",
        "--title",
        "Fix navbar",
        "--list",
        "backlog",
    ]);
    // Positional and --title together are ambiguous.
    assert!(!p
        .cmd(&["ticket", "create", "A", "--title", "B", "--list", "todo"])
        .output()
        .unwrap()
        .status
        .success());

    // `move <id> <list>` (and --to) answer with a receipt in the canonical id.
    assert_eq!(
        p.json(&["ticket", "move", "t1", "in-progress"]),
        json!({ "ok": true, "id": "T-001", "list": "in-progress" })
    );
    let c = p.json(&["comment", "add", "T-001", "Use OAuth"]);
    assert_eq!(
        (c["comment"].as_str(), c["author"].as_str()),
        (Some("c-1"), Some("Tester <t@example.com>"))
    );
    assert_eq!(
        p.json(&["label", "assign", "T-001", "app", "bug", "ios"])["labels"],
        json!(["app", "bug", "ios"])
    );
    assert_eq!(
        p.json(&["label", "remove", "T-001", "bug", "ios"])["labels"],
        json!(["app"])
    );

    let show = p.json(&["ticket", "show", "T-001"]);
    assert_eq!(
        (show["list"].as_str(), show["priority"].as_str()),
        (Some("in-progress"), Some("high"))
    );
    assert_eq!(show["labels"], json!(["app"]));
    assert_eq!(
        (
            show["comments"][0]["by"].as_str(),
            show["comments"][0]["body"].as_str()
        ),
        (Some("Tester <t@example.com>"), Some("Use OAuth"))
    );
    assert_eq!(p.json(&["ticket", "close", "T-001"])["list"], "done");
    assert_eq!(
        ids(&p.json(&["ticket", "list", "--list", "backlog"])),
        ["T-002"]
    );
}

#[test]
fn errors_are_json_on_stdout_with_guidance() {
    let p = Project::with_board("Err");
    let out = p
        .cmd(&["ticket", "show", "T-063", "--json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().unwrap().contains("T-063"));

    // A missing list names the real ones.
    let e = p.json_err(&["ticket", "create", "No list"]);
    assert!(e["error"].as_str().unwrap().contains("todo"), "{e}");
    let e = p.json_err(&["ticket", "create", "x", "--list", "nosuch"]);
    assert!(e["error"].as_str().unwrap().contains("nosuch"), "{e}");
    p.json(&["ticket", "create", "Temp", "--list", "todo"]);
    let e = p.json_err(&["ticket", "move", "T-001", "nowhere"]);
    assert!(e["error"].as_str().unwrap().contains("in-progress"), "{e}");
    // Destructive commands need --yes.
    p.json_err(&["ticket", "delete", "T-001"]);
    p.json(&["ticket", "delete", "T-001", "--yes"]);
}

#[test]
fn config_values_roundtrip_and_validate() {
    let p = Project::with_board("Cfg");
    p.json(&["config", "set", "daemon.port", "9999"]);
    assert_eq!(p.json(&["config", "get", "daemon.port"])["value"], 9999);
    for (v, want) in [
        ("lan", "lan"),
        ("localhost", "local"),
        ("tailscale", "tailscale"),
    ] {
        p.json(&["config", "set", "daemon.expose", v]);
        assert_eq!(p.json(&["config", "get", "daemon.expose"])["value"], want);
    }
    let e = p.json_err(&["config", "set", "daemon.expose", "none"]);
    assert!(e["error"].as_str().unwrap().contains("local"), "{e}");
    let e = p.json_err(&["config", "set", "board.review_list", "nope"]);
    assert!(e["error"].as_str().unwrap().contains("no list"), "{e}");
}

#[test]
fn skill_prints_and_installs() {
    let p = Project::new();
    let skill = p.run(&["skill"]);
    assert!(
        skill.starts_with("---\nname: wipe") && skill.len() < 4000,
        "a short bootstrap: {} bytes",
        skill.len()
    );

    let base = p.path().join("dest");
    let base_s = base.to_str().unwrap();
    assert_eq!(
        p.json(&["skill", "install", "--dir", base_s, "--target", "claude"])["target"],
        "claude"
    );
    assert!(base.join("skills/wipe/SKILL.md").is_file());
    assert!(
        !p.cmd(&["skill", "install", "--dir", base_s])
            .output()
            .unwrap()
            .status
            .success(),
        "no silent overwrite"
    );
    p.json(&["skill", "install", "--dir", base_s, "--force"]);
    assert!(
        p.json(&["skill", "path", "--dir", base_s, "--target", "agents"])["path"]
            .as_str()
            .unwrap()
            .contains("SKILL.md")
    );
    // Auto-detects the agents convention from the project.
    std::fs::create_dir_all(p.path().join(".agents")).unwrap();
    assert_eq!(p.json(&["skill", "install"])["target"], "agents");
}

#[test]
fn comments_and_authorship_corrections_are_audited() {
    let p = Project::with_board("Talk");
    p.json(&["ticket", "create", "X", "--list", "todo"]);
    p.json(&["comment", "add", "T-001", "-b", "first"]);
    p.json(&["comment", "add", "T-001", "-b", "second"]);
    p.json(&["comment", "remove", "T-001", "c-1"]);
    assert_eq!(
        p.json(&["comment", "add", "T-001", "-b", "third"])["comment"],
        "c-3",
        "ids are never reused"
    );
    p.json_err(&["comment", "remove", "T-001", "c-99"]);

    p.json(&["comment", "edit", "T-001", "c-2", "--body", "edited body"]);
    assert_eq!(
        p.json(&["comment", "reattribute", "T-001", "c-2", "--to", "alice"])["to"],
        "alice"
    );
    p.json(&["ticket", "edit", "T-001", "--author", "bob"]);
    let show = p.json(&["ticket", "show", "T-001", "--activity"]);
    assert_eq!(show["author"], "bob", "creator rewritten");
    let c2 = show["comments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "c-2")
        .unwrap();
    assert_eq!(
        (c2["by"].as_str(), c2["body"].as_str()),
        (Some("alice"), Some("edited body"))
    );
    // Corrections leave an audit trail instead of silently overwriting.
    assert!(
        show["activity"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["detail"].as_str().unwrap_or("").contains("c-2")),
        "{show}"
    );

    p.json(&["forum", "post", "--title", "T", "--body", "b"]);
    assert_eq!(
        p.json(&["forum", "edit", "F-1", "--author", "carol"])["to"],
        "carol"
    );
    assert_eq!(p.json(&["forum", "show", "F-1"])["post"]["author"], "carol");
}

#[test]
fn git_commits_are_scoped_attributed_and_linked() {
    let p = Project::with_board("Git");
    p.git_init();
    // Until the first `wipe commit`, writes hint at it - on stderr only.
    let out = p
        .cmd(&["ticket", "create", "Login", "--list", "todo", "--json"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("wipe commit"));
    let _: Value = serde_json::from_slice(&out.stdout).unwrap();
    let d = p.json(&["doctor"]);
    assert!(d["uncommitted_board_files"].as_u64().unwrap() > 0);
    assert!(d["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("wipe commit")));

    // `wipe commit` takes only .wipe/, as the wipe identity, not the repo user.
    std::fs::write(p.path().join("unrelated.txt"), "hi\n").unwrap();
    let c = p.json(&["commit", "-m", "board init", "--author", "claude-dev"]);
    assert!(c["commit"].is_string());
    assert_eq!(p.head_author(), "claude-dev <claude-dev@wipe.local>");
    let st = StdCommand::new("git")
        .current_dir(p.path())
        .args(["status", "--porcelain", "unrelated.txt"])
        .output()
        .unwrap();
    assert!(String::from_utf8(st.stdout)
        .unwrap()
        .contains("?? unrelated.txt"));
    assert!(
        p.json(&["commit"])["commit"].is_null(),
        "nothing left to commit"
    );
    let out = p
        .cmd(&["ticket", "create", "B", "--list", "todo", "--json"])
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("wipe commit"),
        "hint retired"
    );

    // `ticket show` links commits that name the ticket (standalone ids only).
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
    assert_eq!(subjects, ["polish", "fix login redirect (T-001)"]);
    assert!(p
        .json(&["ticket", "show", "T-001", "--no-commits"])
        .get("commits")
        .is_none());

    // Autocommit: every write commits .wipe/ by itself.
    p.run(&["config", "set", "board.autocommit", "true"]);
    p.json(&["ticket", "create", "X", "--list", "todo"]);
    let files = StdCommand::new("git")
        .current_dir(p.path())
        .args(["--no-pager", "show", "--name-only", "--format=", "HEAD"])
        .output()
        .unwrap();
    assert!(String::from_utf8(files.stdout)
        .unwrap()
        .contains("tickets/T-003.json"));

    // Through the npm shim on Windows, doctor explains the argument truncation.
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
fn inbox_subscriptions_and_read_cursor() {
    let p = Project::with_board("Inbox");
    p.json_as("alice", &["ticket", "create", "Task", "--list", "todo"]);
    p.json_as("alice", &["ticket", "assign", "T-001", "bob"]);
    let subs = p.json_as("bob", &["subscriptions"]);
    assert!(
        subs["subscriptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "T-001"),
        "assignees are subscribed"
    );
    p.json_as("alice", &["comment", "add", "T-001", "please start"]);
    p.json_as("bob", &["comment", "add", "T-001", "on it"]);

    // Grouped per ticket; others' changes only, never your own.
    let ib = p.json_as("bob", &["inbox"]);
    let item = &ib["items"][0];
    assert_eq!(
        (item["id"].as_str(), item["why"].as_str()),
        (Some("T-001"), Some("assigned"))
    );
    let events = item["events"].as_array().unwrap();
    assert!(events
        .iter()
        .any(|e| e["event"] == "commented" && e["by"] == "alice" && e["detail"] == "please start"));
    assert!(events.iter().all(|e| e["by"] != "bob"));

    // --unread consumes: the second call is empty.
    assert!(
        p.json_as("bob", &["inbox", "--unread"])["total"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(p.json_as("bob", &["inbox", "--unread"])["total"], 0);

    // Not involved: empty; the board-wide view is not, and needs no identity.
    assert_eq!(p.json_as("carol", &["inbox"])["total"], 0);
    let out = p
        .bare(&["inbox", "--all", "--limit", "1", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let all: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(all["total"].as_u64().unwrap() >= 3 && all.get("identity").is_none());
    assert_eq!(all["items"].as_array().unwrap().len(), 1);
    assert!(all["items"][0].get("why").is_none(), "`board` is implied");
}

#[test]
fn writes_need_an_explicit_identity() {
    let p = Project::with_board("Who");
    p.run(&[
        "identity",
        "use",
        "bilal@example.com",
        "--human",
        "--name",
        "Bilal",
    ]);
    p.run(&["identity", "clear"]);

    let out = p
        .bare(&["ticket", "create", "X", "--list", "todo", "--json"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "no identity, no write");
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let err = v["error"].as_str().unwrap();
    for needle in [
        "no identity chosen",
        "bilal@example.com",
        "wipe identity use <id>",
        "WIPE_AGENT",
        "--agentid",
        "ask your user",
    ] {
        assert!(err.contains(needle), "missing `{needle}` in: {err}");
    }
    assert!(!p.path().join(".wipe/tickets/T-001.json").exists());
    let mut c = p.bare(&["ticket", "create", "X", "--list", "todo", "--json"]);
    c.env("WIPE_STRICT_IDENTITY", "0");
    assert!(
        !c.output().unwrap().status.success(),
        "the legacy opt-out is gone"
    );

    // Reads never need one.
    for read in [
        vec!["status", "--json"],
        vec!["ticket", "list", "--json"],
        vec!["search", "x", "--json"],
        vec!["forum", "list", "--json"],
        vec!["identity", "whoami", "--json"],
    ] {
        assert!(
            p.bare(&read).output().unwrap().status.success(),
            "read {read:?}"
        );
    }

    // Any explicit source unlocks writes; $WIPE_AGENT outranks $WIPE_AUTHOR.
    let mut c = p.cmd(&["ticket", "create", "Y", "--list", "todo", "--json"]);
    c.env("WIPE_AGENT", "agent-7");
    assert!(c.output().unwrap().status.success());
    assert_eq!(p.json(&["ticket", "show", "T-001"])["author"], "agent-7");
    let out = p
        .bare(&[
            "--agentid",
            "gizmo",
            "ticket",
            "create",
            "Z",
            "--list",
            "todo",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "--agentid unlocks writes");
}

#[test]
fn identities_bind_per_session_and_never_leak() {
    let p = Project::with_board("Sessions");
    let run = |envs: &[(&str, &str)], args: &[&str]| {
        let mut c = p.bare(args);
        c.env_remove("WIPE_SESSION");
        for (k, v) in envs {
            c.env(k, v);
        }
        c.output().unwrap()
    };
    let author = |envs: &[(&str, &str)], title: &str| -> Option<String> {
        let out = run(
            envs,
            &["ticket", "create", title, "--list", "todo", "--json"],
        );
        if !out.status.success() {
            return None;
        }
        let id = serde_json::from_slice::<Value>(&out.stdout).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        Some(
            p.json(&["ticket", "show", &id])["author"]
                .as_str()
                .unwrap()
                .to_string(),
        )
    };
    let tab = [("WT_SESSION", "tab-1")];
    assert_eq!(author(&tab, "h0"), None, "a fresh terminal has no identity");
    assert!(run(&tab, &["identity", "use", "bilal@x.com", "--human"])
        .status
        .success());
    assert_eq!(author(&tab, "h1").as_deref(), Some("bilal@x.com"));
    assert_eq!(
        author(&[("WT_SESSION", "tab-2")], "x"),
        None,
        "other terminals don't inherit it"
    );

    // An agent launched from that tab has its own session: no identity until it
    // picks one, and then its own.
    let agent = [
        ("WT_SESSION", "tab-1"),
        ("CLAUDE_CODE_SESSION_ID", "abc"),
        ("CLAUDECODE", "1"),
    ];
    assert_eq!(
        author(&agent, "a0"),
        None,
        "an agent never writes as the human"
    );
    assert!(run(
        &agent,
        &["identity", "use", "claude", "--agent", "--name", "Claude"]
    )
    .status
    .success());
    assert_eq!(author(&agent, "a1").as_deref(), Some("claude"));
    assert_eq!(
        author(&tab, "h2").as_deref(),
        Some("bilal@x.com"),
        "the human's tab is unaffected"
    );
    // A one-command --agentid wins over the session; the list marks the active one.
    let out = run(
        &agent,
        &[
            "--agentid",
            "gizmo",
            "--echo",
            "ticket",
            "create",
            "g",
            "--list",
            "todo",
            "--json",
        ],
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["activity"][0]["actor"],
        "gizmo"
    );
    let list: Value =
        serde_json::from_slice(&run(&agent, &["identity", "list", "--json"]).stdout).unwrap();
    assert_eq!(list["active"], "claude");
    assert!(list["identities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == "claude" && i["kind"] == "agent"));
    assert!(run(&agent, &["identity", "clear"]).status.success());
    assert_eq!(author(&agent, "a2"), None);

    // A harness with no session id of its own is told to use $WIPE_AGENT.
    let out = run(
        &[("WT_SESSION", "tab-1"), ("AI_AGENT", "other")],
        &["identity", "use", "x", "--json"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("WIPE_AGENT"));
}

#[test]
fn delete_trash_restore_duplicate_and_traversal() {
    let p = Project::with_board("Trash");
    p.json(&["ticket", "create", "A", "--list", "todo"]);
    p.json(&["ticket", "create", "B", "--list", "todo"]);
    let d = p.json(&["ticket", "duplicate", "T-001"]);
    assert_eq!(d["id"], "T-003");
    assert!(d["title"].as_str().unwrap().contains("(copy)"));
    assert_eq!(
        ids(&p.json(&["ticket", "list", "--list", "todo"])),
        ["T-001", "T-003", "T-002"],
        "the copy sits after the original"
    );

    assert_eq!(
        p.json(&["ticket", "delete", "T-002", "--yes"])["trashed"],
        true
    );
    p.json_err(&["ticket", "show", "T-002"]);
    assert_eq!(ids(&p.json(&["trash", "list"])["trash"]), ["T-002"]);
    p.json(&["trash", "restore", "t2"]);
    p.json(&["ticket", "show", "T-002"]);
    p.json(&["ticket", "delete", "T-001", "--yes", "--purge"]);
    assert!(
        p.json(&["trash", "list"])["trash"]
            .as_array()
            .unwrap()
            .is_empty(),
        "--purge skips the trash"
    );

    // Crafted ids never escape their directory.
    let board = p.path().join(".wipe/board.json");
    p.json_err(&["ticket", "delete", "../board", "--yes"]);
    assert_eq!(p.json(&["trash", "purge", "../../board"])["purged"], false);
    assert!(board.is_file());
}

/// Knowledge one agent posts is found by others, each a separate process (as in
/// real harnesses) - the forum's on-disk index must stay correct across them.
#[test]
fn multi_agent_forum_collaboration() {
    let p = Project::with_board("Calc");
    let dec = p.json_as(
        "architect",
        &[
            "forum",
            "post",
            "-t",
            "Money handling",
            "-b",
            "All money is integer cents; never floats.",
            "--label",
            "decision",
        ],
    );
    assert_eq!(dec["id"], "F-1");
    p.json_as(
        "architect",
        &[
            "forum",
            "reply",
            "F-1",
            "-b",
            "Gotcha: big ints serialize as strings.",
            "--label",
            "gotcha",
        ],
    );

    let found = p.json_as("coder", &["forum", "search", "cents|float"]);
    assert_eq!(found[0]["thread_id"], "F-1");
    p.json_as(
        "coder",
        &["forum", "reply", "F-1", "-b", "Confirmed. Using i64 cents."],
    );
    p.json_as(
        "coder",
        &[
            "forum",
            "post",
            "-t",
            "Rounding",
            "-b",
            "Round half-up at display only.",
            "--label",
            "rule",
        ],
    );

    let authors: std::collections::HashSet<String> = p
        .json_as("reviewer", &["forum", "search", "."])
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["author"].as_str().unwrap().to_string())
        .collect();
    assert!(
        authors.contains("architect") && authors.contains("coder"),
        "{authors:?}"
    );
    assert_eq!(
        p.json_as("reviewer", &["forum", "search", "--scope", "F-1"])
            .as_array()
            .unwrap()
            .len(),
        3
    );
    // The unified search finds forum knowledge too.
    let s = p.json_as("reviewer", &["search", "cents"]);
    assert_eq!(
        s["posts"][0]["id"].as_str().unwrap().split('.').next(),
        Some("F-1")
    );
}
