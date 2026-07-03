//! Validates wipe's core git-native promise: because each ticket is its own file
//! and card content is separated from board structure, concurrent edits to
//! *different* tickets merge cleanly with no conflicts.

use std::path::Path;
use std::process::Command;

use wipe_core::ops::{self, NewTicket};
use wipe_core::Store;

fn git(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn git_ok(root: &Path, args: &[&str]) {
    let out = git(root, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

/// Two branches each edit a *different* ticket file; merging is conflict-free.
#[test]
fn concurrent_edits_to_different_tickets_merge_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    git_ok(root, &["init", "-q", "-b", "main"]);
    git_ok(root, &["config", "user.email", "t@example.com"]);
    git_ok(root, &["config", "user.name", "Tester"]);

    let store = Store::init(root, "Merge Test", now()).unwrap();
    ops::create_ticket(
        &store,
        NewTicket {
            title: "Alpha".into(),
            ..Default::default()
        },
        "tester",
        now(),
    )
    .unwrap();
    ops::create_ticket(
        &store,
        NewTicket {
            title: "Beta".into(),
            ..Default::default()
        },
        "tester",
        now(),
    )
    .unwrap();
    git_ok(root, &["add", "-A"]);
    git_ok(root, &["commit", "-q", "-m", "seed two tickets"]);

    // Branch edit-alpha: change T-1's title only (touches tickets/T-1.json).
    git_ok(root, &["checkout", "-q", "-b", "edit-alpha"]);
    let mut t1 = store.load_ticket("T-1").unwrap();
    t1.title = "Alpha edited".into();
    store.save_ticket(&t1).unwrap();
    git_ok(root, &["commit", "-am", "edit alpha"]);

    // Branch edit-beta from main: change T-2's title only (touches tickets/T-2.json).
    git_ok(root, &["checkout", "-q", "main"]);
    git_ok(root, &["checkout", "-q", "-b", "edit-beta"]);
    let mut t2 = store.load_ticket("T-2").unwrap();
    t2.title = "Beta edited".into();
    store.save_ticket(&t2).unwrap();
    git_ok(root, &["commit", "-am", "edit beta"]);

    // Merge both branches into main - must be conflict-free.
    git_ok(root, &["checkout", "-q", "main"]);
    git_ok(root, &["merge", "-q", "--no-edit", "edit-alpha"]);
    let merge = git(root, &["merge", "--no-edit", "edit-beta"]);
    assert!(
        merge.status.success(),
        "second merge conflicted: {}",
        String::from_utf8_lossy(&merge.stderr)
    );

    // Both independent edits survive.
    assert_eq!(store.load_ticket("T-1").unwrap().title, "Alpha edited");
    assert_eq!(store.load_ticket("T-2").unwrap().title, "Beta edited");
}
