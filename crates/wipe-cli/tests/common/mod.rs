//! Shared harness for the CLI integration tests: a throwaway board in a temp dir
//! driven through the real `wipe` binary, with a deterministic identity and an
//! isolated global config so tests never touch the developer's machine state.

#![allow(dead_code)] // each test crate uses a different subset

use std::io::Write as _;
use std::path::Path;
use std::process::{Command as StdCommand, Output, Stdio};

use assert_cmd::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

/// Env vars that could leak an identity (or a session) from the developer's /
/// agent's own environment into a test.
const AMBIENT: &[&str] = &[
    "WIPE_AGENT",
    "WIPE_AUTHOR",
    "WIPE_SESSION",
    "WIPE_STRICT_IDENTITY",
    "WIPE_NPM_SHIM",
    "WT_SESSION",
    "TERM_SESSION_ID",
    "ITERM_SESSION_ID",
    "TMUX_PANE",
    "STY",
    "CLAUDE_CODE_SESSION_ID",
    "AI_AGENT",
    "CLAUDECODE",
];

/// A throwaway wipe project rooted in a temp dir, with a deterministic identity.
pub struct Project {
    pub dir: TempDir,
}

impl Project {
    pub fn new() -> Self {
        Project {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// A project with an initialized board named `name`.
    pub fn with_board(name: &str) -> Self {
        let p = Project::new();
        p.run(&["init", ".", "--yes", "--name", name]);
        p
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Run a raw `git` command in the project dir, asserting success.
    pub fn git(&self, args: &[&str]) {
        let ok = StdCommand::new("git")
            .current_dir(self.dir.path())
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// `git init` plus a local user, so commits work.
    pub fn git_init(&self) {
        self.git(&["init", "-q"]);
        self.git(&["config", "user.email", "t@example.com"]);
        self.git(&["config", "user.name", "Tester"]);
    }

    /// The author of the current `HEAD` commit as `Name <email>`.
    pub fn head_author(&self) -> String {
        let out = StdCommand::new("git")
            .current_dir(self.dir.path())
            .args(["--no-pager", "log", "-1", "--format=%an <%ae>"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// A `wipe` invocation with NO identity of any kind: the ambient identity /
    /// session env is scrubbed and a unique `$WIPE_SESSION` pins the session key.
    pub fn bare(&self, args: &[&str]) -> StdCommand {
        let mut c = StdCommand::cargo_bin("wipe").unwrap();
        c.current_dir(self.dir.path());
        for v in AMBIENT {
            c.env_remove(v);
        }
        c.env("WIPE_CONFIG_DIR", self.dir.path());
        c.env("WIPE_SESSION", "test-session");
        // Keep the once-a-day update check off the network.
        c.env("WIPE_NO_UPDATE_CHECK", "1");
        c.args(args);
        c
    }

    /// Build a `wipe` invocation rooted at this project with a fixed author.
    pub fn cmd(&self, args: &[&str]) -> StdCommand {
        let mut c = self.bare(args);
        c.env("WIPE_AUTHOR", "Tester <t@example.com>");
        c
    }

    /// Run a command, assert success, and return stdout as a String.
    pub fn run(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        assert!(
            out.status.success(),
            "command {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// Run a command with `--json` and parse stdout as JSON.
    pub fn json(&self, args: &[&str]) -> Value {
        let mut v = args.to_vec();
        v.push("--json");
        let stdout = self.run(&v);
        serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("bad json from {args:?}: {e}\n{stdout}"))
    }

    /// Run a `--json` command as a specific author (each `wipe` call is a distinct
    /// process, mirroring how independent agents drive the same board).
    pub fn json_as(&self, author: &str, args: &[&str]) -> Value {
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

    /// Run a command feeding `stdin`, returning the raw output.
    pub fn with_stdin(&self, args: &[&str], stdin: &str) -> Output {
        let mut c = self.cmd(args);
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    /// Run a command expecting failure; returns the parsed `--json` error object.
    pub fn json_err(&self, args: &[&str]) -> Value {
        let mut v = args.to_vec();
        v.push("--json");
        let out = self.cmd(&v).output().unwrap();
        assert!(
            !out.status.success(),
            "command {args:?} unexpectedly succeeded"
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "bad json error from {args:?}: {e}\n{}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }
}
