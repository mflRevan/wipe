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

    /// Build a `wipe` invocation rooted at this project with a fixed author.
    fn cmd(&self, args: &[&str]) -> StdCommand {
        let mut c = StdCommand::cargo_bin("wipe").unwrap();
        c.current_dir(self.dir.path());
        c.env("WIPE_AUTHOR", "Tester <t@example.com>");
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
