//! End-to-end CLI tests driving the real `wipe` binary against throwaway boards.
//!
//! These double as the "mock project + predefined flow" harness: they set up a
//! board, run the exact commands an agent would run, and assert on both the
//! human output and the `--json` contract. Reuse [`Project`] for new flows.

use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

/// A throwaway wipe project rooted in a temp dir, with a deterministic identity.
struct Project {
    dir: TempDir,
}

impl Project {
    fn new() -> Self {
        Project {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Build a `wipe` invocation rooted at this project with a fixed author and
    /// an isolated global-config dir (so tests never read/write the real one).
    fn cmd(&self, args: &[&str]) -> StdCommand {
        let mut c = StdCommand::cargo_bin("wipe").unwrap();
        c.current_dir(self.dir.path());
        c.env("WIPE_AUTHOR", "Tester <t@example.com>");
        c.env("WIPE_CONFIG_DIR", self.dir.path());
        c.args(args);
        c
    }

    /// Run a command, assert success, and return stdout as a String.
    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        assert!(
            out.status.success(),
            "command {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// Run a command with `--json` and parse stdout as JSON.
    fn json(&self, args: &[&str]) -> Value {
        let mut v = args.to_vec();
        v.push("--json");
        let stdout = self.run(&v);
        serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("bad json from {args:?}: {e}\n{stdout}"))
    }

    /// Run a `--json` command as a specific author (each `wipe` call is a distinct
    /// process, mirroring how independent agents drive the same board).
    fn json_as(&self, author: &str, args: &[&str]) -> Value {
        let mut v = args.to_vec();
        v.push("--json");
        let mut c = self.cmd(&v);
        c.env("WIPE_AUTHOR", author);
        let out = c.output().unwrap();
        assert!(
            out.status.success(),
            "command {args:?} as {author} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).unwrap();
        serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("bad json from {args:?}: {e}\n{stdout}"))
    }
}

#[test]
fn init_creates_board_and_status_shows_lists() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Demo"]);
    assert!(p.path().join(".wipe/board.json").is_file());
    let status = p.json(&["status"]);
    assert_eq!(status["board"], "Demo");
    let lists = status["lists"].as_array().unwrap();
    assert_eq!(lists.len(), 4);
    assert_eq!(lists[0]["list"], "backlog");
}

#[test]
fn full_agent_flow() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Flow"]);

    // Create two tickets; IDs are allocated deterministically.
    let t1 = p.json(&[
        "ticket",
        "create",
        "--title",
        "Add login",
        "--priority",
        "high",
    ]);
    assert_eq!(t1["id"], "T-1");
    let t2 = p.json(&["ticket", "create", "--title", "Fix navbar"]);
    assert_eq!(t2["id"], "T-2");

    // Move, comment, label.
    p.json(&["ticket", "move", "T-1", "--to", "in-progress"]);
    let c = p.json(&["comment", "add", "T-1", "--body", "Use OAuth"]);
    assert_eq!(c["comment"], "c-1");
    assert_eq!(c["author"], "Tester <t@example.com>");
    p.json(&["label", "assign", "T-1", "needs-review"]);

    // Verify ticket state via JSON.
    let show = p.json(&["ticket", "show", "T-1"]);
    assert_eq!(show["list"], "in-progress");
    assert_eq!(show["labels"][0], "needs-review");
    assert_eq!(show["comments"][0]["body"], "Use OAuth");

    // Close moves to done.
    let closed = p.json(&["ticket", "close", "T-1"]);
    assert_eq!(closed["list"], "done");

    // Filtered listing.
    let backlog = p.json(&["ticket", "list", "--list", "backlog"]);
    let arr = backlog.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "T-2");
}

#[test]
fn json_error_object_and_nonzero_exit_on_missing_ticket() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Err"]);
    let out = p
        .cmd(&["ticket", "show", "T-99", "--json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().unwrap().contains("T-99"));
}

#[test]
fn delete_requires_confirmation() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Del"]);
    p.json(&["ticket", "create", "--title", "Temp"]);
    // Without --yes it must refuse and exit non-zero.
    let refused = p.cmd(&["ticket", "delete", "T-1"]).output().unwrap();
    assert!(!refused.status.success());
    // With --yes it succeeds.
    p.json(&["ticket", "delete", "T-1", "--yes"]);
    let missing = p
        .cmd(&["ticket", "show", "T-1", "--json"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
}

#[test]
fn discovers_board_from_subdirectory() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Nested"]);
    let sub = p.path().join("src").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    let mut c = StdCommand::cargo_bin("wipe").unwrap();
    c.current_dir(&sub)
        .env("WIPE_AUTHOR", "T <t@e.com>")
        .args(["status", "--json"]);
    let out = c.output().unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["board"], "Nested");
}

#[test]
fn config_roundtrip() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Cfg"]);
    p.json(&["config", "set", "daemon.port", "9999"]);
    let got = p.json(&["config", "get", "daemon.port"]);
    assert_eq!(got["value"], 9999);
    p.json(&["config", "set", "daemon.expose", "tailscale"]);
    let expose = p.json(&["config", "get", "daemon.expose"]);
    assert_eq!(expose["value"], "tailscale");
}

#[test]
fn skill_is_embedded() {
    let p = Project::new();
    let out = p.run(&["skill"]);
    assert!(out.contains("wipe - agent operating guide"));
    // `skill show` is the explicit form of the same output.
    assert!(p.run(&["skill", "show"]).contains("agent operating guide"));
}

#[test]
fn init_empty_starter_has_no_lists() {
    let p = Project::new();
    let v = p.json(&[
        "init",
        ".",
        "--name",
        "Blank",
        "--yes",
        "--starter",
        "empty",
    ]);
    assert_eq!(v["starter"], "empty");
    let status = p.json(&["status"]);
    assert!(status["lists"].as_array().unwrap().is_empty());
}

#[test]
fn skill_install_writes_file_and_respects_force() {
    let p = Project::new();
    let base = p.path().join("dest");
    let base_s = base.to_str().unwrap();

    let v = p.json(&["skill", "install", "--dir", base_s, "--target", "claude"]);
    assert_eq!(v["target"], "claude");
    assert!(base.join("skills/wipe/SKILL.md").is_file());

    // A second install without --force fails; with --force it succeeds.
    let again = p
        .cmd(&["skill", "install", "--dir", base_s])
        .output()
        .unwrap();
    assert!(!again.status.success());
    p.json(&["skill", "install", "--dir", base_s, "--force"]);

    // `skill path` reports the location without writing.
    let path = p.json(&["skill", "path", "--dir", base_s, "--target", "agents"]);
    assert!(path["path"].as_str().unwrap().contains("SKILL.md"));
}

#[test]
fn skill_install_auto_detects_agents_dir() {
    let p = Project::new();
    std::fs::create_dir_all(p.path().join(".agents")).unwrap();
    let v = p.json(&["skill", "install"]);
    assert_eq!(v["target"], "agents");
    assert!(p.path().join(".agents/skills/wipe/SKILL.md").is_file());
}

#[test]
fn global_config_roundtrip() {
    let p = Project::new();
    p.json(&["config", "--global", "set", "autoserve", "true"]);
    p.json(&["config", "--global", "set", "starter", "empty"]);
    assert_eq!(
        p.json(&["config", "--global", "get", "autoserve"])["value"],
        true
    );
    assert_eq!(
        p.json(&["config", "--global", "get", "starter"])["value"],
        "empty"
    );
    // The stored global default then drives a non-interactive init.
    let v = p.json(&["init", ".", "--name", "G", "--yes"]);
    assert_eq!(v["starter"], "empty");
    assert_eq!(v["autoserve"], true);
}

#[test]
fn forum_threads_replies_and_search() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "F"]);

    let t = p.json(&[
        "forum",
        "post",
        "-t",
        "Auth",
        "-b",
        "use OAuth 2.1",
        "--label",
        "decision",
    ]);
    assert_eq!(t["id"], "F-1");
    let r = p.json(&[
        "forum",
        "reply",
        "F-1",
        "-b",
        "watch token races",
        "--label",
        "gotcha",
    ]);
    assert_eq!(r["id"], "F-1.1");
    let nested = p.json(&["forum", "reply", "F-1.1", "-b", "use a mutex"]);
    assert_eq!(nested["id"], "F-1.1.1");

    // regex over bodies
    let hits = p.json(&["forum", "search", "token|mutex"]);
    assert_eq!(hits.as_array().unwrap().len(), 2);
    // by label
    let g = p.json(&["forum", "search", "--label", "gotcha"]);
    assert_eq!(g.as_array().unwrap().len(), 1);
    assert_eq!(g[0]["id"], "F-1.1");
    // titles only
    let titles = p.json(&["forum", "search", "auth", "--titles"]);
    assert_eq!(titles.as_array().unwrap().len(), 1);
    assert_eq!(titles[0]["id"], "F-1");
    // scoped to a subtree
    let scoped = p.json(&["forum", "search", "--scope", "F-1"]);
    assert_eq!(scoped.as_array().unwrap().len(), 3);
}

#[test]
fn forum_delete_removes_subtree_and_requires_yes() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "F"]);
    p.json(&["forum", "post", "-t", "T", "-b", "root"]);
    p.json(&["forum", "reply", "F-1", "-b", "a"]); // F-1.1
    p.json(&["forum", "reply", "F-1.1", "-b", "aa"]); // F-1.1.1
    p.json(&["forum", "reply", "F-1", "-b", "b"]); // F-1.2

    // Delete refuses without --yes.
    assert!(!p
        .cmd(&["forum", "delete", "F-1.1"])
        .output()
        .unwrap()
        .status
        .success());
    p.json(&["forum", "delete", "F-1.1", "--yes"]);

    let ids: Vec<String> = p
        .json(&["forum", "search"])
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"F-1".to_string()));
    assert!(ids.contains(&"F-1.2".to_string()));
    assert!(!ids.iter().any(|i| i.starts_with("F-1.1")));
}

#[test]
fn checklist_and_criteria_are_independent_surfaces() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "C"]);
    p.json(&["ticket", "create", "-t", "Ship it"]); // T-1

    // Two independent tickable lists with their own ID namespaces.
    assert_eq!(
        p.json(&["checklist", "add", "T-1", "-t", "write code"])["item"],
        "ck-1"
    );
    assert_eq!(
        p.json(&["criteria", "add", "T-1", "-t", "tests pass"])["item"],
        "ac-1"
    );
    assert_eq!(
        p.json(&["criteria", "add", "T-1", "-t", "docs updated"])["item"],
        "ac-2"
    );

    // A reviewer accepts one criterion; the checklist is untouched.
    p.json(&["criteria", "check", "T-1", "ac-1"]);
    let ac = p.json(&["criteria", "list", "T-1"]);
    let items = ac["acceptance"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["done"], true);
    assert_eq!(items[1]["done"], false);
    let ck = p.json(&["checklist", "list", "T-1"]);
    assert_eq!(ck["checklist"][0]["done"], false);

    // A ck- id is not addressable through the criteria surface.
    assert!(!p
        .cmd(&["criteria", "check", "T-1", "ck-1"])
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn comments_can_be_added_and_removed() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "C"]);
    p.json(&["ticket", "create", "-t", "Talk"]); // T-1
    assert_eq!(
        p.json(&["comment", "add", "T-1", "-b", "first"])["comment"],
        "c-1"
    );
    p.json(&["comment", "add", "T-1", "-b", "second"]); // c-2

    p.json(&["comment", "remove", "T-1", "c-1"]);
    let listed = p.json(&["comment", "list", "T-1"]);
    let ids: Vec<String> = listed["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec!["c-2"]);

    // A fresh comment still advances the counter (no id reuse after deletion).
    assert_eq!(
        p.json(&["comment", "add", "T-1", "-b", "third"])["comment"],
        "c-3"
    );
    // Removing a missing comment is an error, not a panic.
    assert!(!p
        .cmd(&["comment", "remove", "T-1", "c-99"])
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn wipe_agent_env_outranks_wipe_author() {
    // The per-terminal $WIPE_AGENT identity must win over $WIPE_AUTHOR (which the
    // test harness always sets), so multi-agent attribution is race-free.
    let p = Project::new();
    p.run(&["init", ".", "--name", "A"]);
    p.json(&["ticket", "create", "-t", "X"]); // T-1
    let mut c = p.cmd(&["comment", "add", "T-1", "-b", "hi", "--json"]);
    c.env("WIPE_AGENT", "agent-7");
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "comment add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let listed = p.json(&["comment", "list", "T-1"]);
    assert_eq!(listed["comments"][0]["author"], "agent-7");
}

#[test]
fn ticket_delete_rejects_path_traversal() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Safe"]);
    p.json(&["ticket", "create", "-t", "Real"]); // T-1
    assert!(p.path().join(".wipe/board.json").is_file());

    // A crafted id that would escape tickets/ must fail, not delete board.json.
    let out = p
        .cmd(&["ticket", "delete", "../board", "--yes", "--json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(p.path().join(".wipe/board.json").is_file());
}

/// Multi-agent collaboration through the forum, each agent a separate process
/// (as real harnesses are). This is the flagship guard that project knowledge
/// posted by one agent is discoverable by another, and that the on-disk search
/// cache stays correct ACROSS processes. Bugs here (stale cache, cross-author
/// search misses, subtree corruption) surface as failures.
#[test]
fn multi_agent_forum_collaboration() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Calc Service"]);

    // The architect records a durable decision and a gotcha (compounding knowledge).
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
            "Gotcha: JSON serializes big ints as strings; parse carefully.",
            "--label",
            "gotcha",
        ],
    );

    // A supervisor files work that points at the forum.
    p.json(&[
        "ticket",
        "create",
        "--title",
        "Implement add()",
        "--list",
        "todo",
        "--body",
        "Follow the money rules in the forum.",
    ]);

    // The coder starts a FRESH process and, as the skill teaches, searches the
    // forum first - and must find the architect's decision (cross-agent, cross-process).
    let rules = p.json_as("coder", &["forum", "search", "cents|float|money"]);
    let rules = rules.as_array().unwrap();
    assert!(
        !rules.is_empty(),
        "coder must discover the architect's decision via search"
    );
    assert!(rules.iter().any(|r| r["author"] == "architect"));
    let thread = rules[0]["thread_id"].as_str().unwrap().to_string();
    assert_eq!(thread, "F-1");

    // The coder confirms understanding in-thread and records a NEW discovered rule.
    p.json_as(
        "coder",
        &[
            "forum",
            "reply",
            "F-1",
            "-b",
            "Confirmed. Using i64 cents in add().",
        ],
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

    // A reviewer (third process) searches and finds insights from BOTH agents -
    // the forum is the shared, compounding project memory.
    let all_rules = p.json_as("reviewer", &["forum", "search", "--label", "decision"]);
    assert!(all_rules
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["author"] == "architect"));
    let authors: std::collections::HashSet<String> = p
        .json_as("reviewer", &["forum", "search", "."])
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["author"].as_str().unwrap().to_string())
        .collect();
    assert!(
        authors.contains("architect") && authors.contains("coder"),
        "the reviewer should see contributions from every agent: {authors:?}"
    );

    // The reviewer can scope a search to one thread and dive in.
    let in_thread = p.json_as("reviewer", &["forum", "search", "--scope", "F-1"]);
    assert_eq!(in_thread.as_array().unwrap().len(), 3); // decision + gotcha + coder confirm
}

/// Deterministic mirror of the agent-to-agent supervision loop (no LLM), so CI
/// guards the exact collaboration protocol the live opencode harness exercises.
#[test]
fn supervision_protocol_offline() {
    let p = Project::new();
    p.run(&["init", ".", "--name", "Calc Service"]);

    // Supervisor files a spec ticket into `todo`.
    let filed = p.json(&[
        "ticket",
        "create",
        "--title",
        "Implement add",
        "--list",
        "todo",
        "--body",
        "Create calc.py defining add(a, b).",
    ]);
    assert_eq!(filed["id"], "T-1");

    // Subordinate discovers assigned work purely via --json.
    let todo = p.json(&["ticket", "list", "--list", "todo"]);
    let assigned = todo.as_array().unwrap();
    assert_eq!(assigned.len(), 1);
    let id = assigned[0]["id"].as_str().unwrap().to_string();

    // ...reads the spec...
    let spec = p.json(&["ticket", "show", &id]);
    assert!(spec["body"].as_str().unwrap().contains("calc.py"));

    // ...reports back and advances the ticket.
    p.json(&[
        "comment",
        "add",
        &id,
        "--body",
        "Implemented add(a,b) in calc.py",
    ]);
    p.json(&["ticket", "move", &id, "--to", "done"]);

    // Supervisor verifies the acceptance state.
    let done = p.json(&["ticket", "show", &id]);
    assert_eq!(done["list"], "done");
    assert!(!done["comments"].as_array().unwrap().is_empty());
}

/// Identity resolution: a non-git board never attributes to "unknown"; a
/// session-bound identity and a one-shot `--agentid` override both attribute
/// correctly; and `identity list` surfaces the agent as active.
#[test]
fn identity_session_and_agentid_override() {
    let p = Project::new();
    p.run(&["init", "--yes", "--name", "Ids"]);

    let base = p.path().to_path_buf();
    // Run with NO WIPE_AUTHOR, a fixed session, and isolated global config, so we
    // exercise the real resolution chain rather than the env override.
    let run = |args: &[&str]| -> Value {
        let mut v = args.to_vec();
        v.push("--json");
        let mut c = StdCommand::cargo_bin("wipe").unwrap();
        c.current_dir(&base);
        c.env_remove("WIPE_AUTHOR");
        c.env("WIPE_CONFIG_DIR", &base);
        c.env("WIPE_SESSION", "sess-A");
        c.args(&v);
        let out = c.output().unwrap();
        assert!(
            out.status.success(),
            "command {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };

    // No session identity yet: author is a real default, never "unknown"/empty.
    let t1 = run(&["ticket", "create", "-t", "one"]);
    let a1 = t1["activity"][0]["actor"].as_str().unwrap();
    assert!(
        !a1.is_empty() && a1 != "unknown",
        "default author was '{a1}'"
    );

    // Bind a session identity; the next ticket is authored by it.
    run(&["identity", "use", "claude", "--agent", "--name", "Claude"]);
    let t2 = run(&["ticket", "create", "-t", "two"]);
    assert_eq!(t2["activity"][0]["actor"], "claude");

    // whoami reflects the bound identity.
    assert_eq!(run(&["identity", "whoami"])["identity"], "claude");

    // A single-command --agentid override wins over the session.
    let t3 = run(&["--agentid", "gizmo", "ticket", "create", "-t", "three"]);
    assert_eq!(t3["activity"][0]["actor"], "gizmo");

    // The agent shows up in the identity list, marked active.
    let list = run(&["identity", "list"]);
    assert_eq!(list["active"], "claude");
    let has_agent = list["identities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == "claude" && i["kind"] == "agent");
    assert!(has_agent, "claude should be listed as an agent");

    // Clearing the session reverts to the default author.
    run(&["identity", "clear"]);
    let t4 = run(&["ticket", "create", "-t", "four"]);
    assert_ne!(t4["activity"][0]["actor"], "claude");
}
