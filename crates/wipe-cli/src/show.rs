//! `wipe ticket show`: one ticket, sized for a reader with a context budget.
//!
//! By default it is everything needed to work on the ticket and nothing that
//! grows without bound: the header on two lines, the body, checklists,
//! attachments by name, the newest few comments, and the latest commits naming
//! it. Long text is clipped with a count of what was left out, and each cut ends
//! in the flag that reveals it (`--comments all`, `--full`, `--activity`).

use chrono::{DateTime, Datelike, Utc};
use serde_json::{json, Map, Value};

use wipe_core::git::CommitInfo;
use wipe_core::model::{ChecklistItem, Ticket};

use crate::output::{dim, id_style};
use crate::safe;

/// Body characters shown by default.
pub const BODY_MAX: usize = 2400;
/// Characters per comment shown by default.
pub const COMMENT_MAX: usize = 1000;
/// Comments shown by default (the newest).
pub const COMMENTS_DEFAULT: usize = 3;
/// Checklist / criteria items shown per list by default.
const ITEMS_MAX: usize = 30;
/// Commits shown by default.
const COMMITS_DEFAULT: usize = 3;

/// What to include.
pub struct Opts {
    /// Newest comments to include (`usize::MAX` = all).
    pub comments: usize,
    /// No clipping, original wording, activity log, every commit.
    pub full: bool,
    /// Include the activity log.
    pub activity: bool,
    /// Include commits.
    pub commits: bool,
}

/// A compact local timestamp: `10-07 14:02` this year, `2025-10-07` before.
pub fn when(ts: DateTime<Utc>) -> String {
    if ts.year() == Utc::now().year() {
        ts.format("%m-%d %H:%M").to_string()
    } else {
        ts.format("%Y-%m-%d").to_string()
    }
}

/// RFC-3339 to the second (`2026-10-07T14:02:25Z`).
pub fn iso(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// `5.7 MB`, `912 KB`, `40 B`.
pub fn bytes(n: u64) -> String {
    match n {
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{} KB", n >> 10),
        n => format!("{n} B"),
    }
}

fn clip(s: &str, max: usize, full: bool) -> (String, usize) {
    if full {
        (safe::clean(s).into_owned(), 0)
    } else {
        safe::clip(s, max)
    }
}

fn progress(items: &[ChecklistItem]) -> String {
    format!(
        "{}/{}",
        items.iter().filter(|i| i.done).count(),
        items.len()
    )
}

/// The one-line header facts: `in-progress · high · [sky] · @claude · ...`.
fn facts(t: &Ticket, list: Option<&str>, blockers: &[&str]) -> Vec<String> {
    let mut f = Vec::new();
    if let Some(l) = list {
        f.push(l.to_string());
    }
    if let Some(p) = &t.priority {
        f.push(safe::line(p));
    }
    if !t.labels.is_empty() {
        f.push(format!("[{}]", safe::line(&t.labels.join(","))));
    }
    for a in &t.assignees {
        f.push(format!("@{}", safe::line(a)));
    }
    if !blockers.is_empty() {
        f.push(format!("blocked by {}", blockers.join(",")));
    }
    if let Some(by) = t.creator() {
        f.push(format!("by {} {}", safe::line(by), when(t.created)));
    }
    if t.updated != t.created {
        f.push(format!("updated {}", when(t.updated)));
    }
    if let Some(old) = &t.legacy_id {
        f.push(format!("was {old}"));
    }
    f
}

/// Print the ticket as text.
pub fn print(t: &Ticket, list: Option<&str>, commits: &[CommitInfo], o: &Opts) {
    let blockers: Vec<&str> = t.blocked_by().collect();
    println!("{}  {}", id_style(&t.id), safe::line(&t.title));
    println!("{}", dim(&facts(t, list, &blockers).join(" · ")));
    let mut more: Vec<String> = Vec::new();

    if !t.body.is_empty() {
        let (body, cut) = clip(&t.body, BODY_MAX, o.full);
        println!("\n{body}");
        if cut > 0 {
            println!("{}", dim(&format!("… {cut} more chars of the body")));
            more.push("--full".into());
        }
    }
    if let Some(orig) = &t.original {
        if o.full {
            println!(
                "\n{}",
                dim(&format!(
                    "original wording by {} (before {} rewrote it):",
                    orig.author, orig.rewritten_by
                ))
            );
            println!("  {}", safe::line(&orig.title));
            for l in safe::clean(&orig.body).lines() {
                println!("  {l}");
            }
        }
    }
    for (name, items) in [("checklist", &t.checklist), ("criteria", &t.acceptance)] {
        if items.is_empty() {
            continue;
        }
        println!("\n{}", dim(&format!("{name} {}:", progress(items))));
        let cap = if o.full { usize::MAX } else { ITEMS_MAX };
        for i in items.iter().take(cap) {
            let mark = if i.done { "[x]" } else { "[ ]" };
            println!(
                "  {mark} {} {}",
                i.id,
                safe::clip(&i.text, 200).0.replace('\n', " ")
            );
        }
        if items.len() > cap {
            println!("  {}", dim(&format!("… {} more", items.len() - cap)));
            more.push("--full".into());
        }
    }
    if !t.attachments.is_empty() {
        let names: Vec<String> = t
            .attachments
            .iter()
            .map(|a| format!("{} ({})", safe::line(&a.name), bytes(a.size)))
            .collect();
        println!("\n{}", dim(&format!("attachments: {}", names.join(", "))));
    }
    let total = t.comments.len();
    if total > 0 && o.comments > 0 {
        let shown = total.min(o.comments);
        let header = if shown < total {
            more.push("--comments all".into());
            format!("{total} comments, newest {shown}:")
        } else {
            format!("{total} comment{}:", if total == 1 { "" } else { "s" })
        };
        println!("\n{}", dim(&header));
        for c in &t.comments[total - shown..] {
            println!(
                "{} {} {}",
                c.id,
                safe::line(&c.author),
                dim(&when(c.created))
            );
            let (body, cut) = clip(&c.body, COMMENT_MAX, o.full);
            for l in body.lines() {
                println!("  {l}");
            }
            if cut > 0 {
                println!("  {}", dim(&format!("… {cut} more chars")));
                if !more.contains(&"--full".to_string()) {
                    more.push("--full".into());
                }
            }
        }
    } else if total > 0 {
        println!("\n{}", dim(&format!("{total} comments (hidden)")));
    }
    if o.activity || o.full {
        if !t.activity.is_empty() {
            println!("\n{}", dim("activity:"));
            for a in &t.activity {
                let (kind, detail) = wipe_core::events::describe(&a.kind, &a.detail);
                println!(
                    "  {}  {} {} {}",
                    dim(&when(a.ts)),
                    safe::line(&a.actor),
                    kind,
                    safe::line(&detail)
                );
            }
        }
    } else if !t.activity.is_empty() {
        more.push("--activity".into());
    }
    if o.commits && !commits.is_empty() {
        let cap = if o.full { usize::MAX } else { COMMITS_DEFAULT };
        println!("\n{}", dim("commits:"));
        for c in commits.iter().take(cap) {
            println!(
                "  {}  {}  {}",
                c.short,
                safe::line(&c.subject),
                dim(&safe::line(&c.author_name))
            );
        }
        if commits.len() > cap {
            println!("  {}", dim(&format!("… {} more", commits.len() - cap)));
        }
    }
    more.dedup();
    if !more.is_empty() {
        println!("{}", dim(&format!("\nmore: {}", more.join(" · "))));
    }
}

fn items_json(items: &[ChecklistItem], full: bool) -> Value {
    let cap = if full { usize::MAX } else { ITEMS_MAX };
    json!(items
        .iter()
        .take(cap)
        .map(|i| json!({ "id": i.id, "text": safe::clean(&i.text), "done": i.done }))
        .collect::<Vec<_>>())
}

/// The ticket as compact JSON (same selection as [`print`]). Clipped strings
/// carry their full length in a sibling `*_chars` field.
pub fn json(t: &Ticket, list: Option<&str>, commits: &[CommitInfo], o: &Opts) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(t.id));
    m.insert("title".into(), json!(safe::line(&t.title)));
    if let Some(l) = list {
        m.insert("list".into(), json!(l));
    }
    if let Some(p) = &t.priority {
        m.insert("priority".into(), json!(p));
    }
    if !t.labels.is_empty() {
        m.insert("labels".into(), json!(t.labels));
    }
    if !t.assignees.is_empty() {
        m.insert("assignees".into(), json!(t.assignees));
    }
    let blockers: Vec<&str> = t.blocked_by().collect();
    if !blockers.is_empty() {
        m.insert("blocked_by".into(), json!(blockers));
    }
    if let Some(by) = t.creator() {
        m.insert("author".into(), json!(by));
    }
    m.insert("created".into(), json!(iso(t.created)));
    m.insert("updated".into(), json!(iso(t.updated)));
    if let Some(old) = &t.legacy_id {
        m.insert("legacy_id".into(), json!(old));
    }
    if !t.body.is_empty() {
        let (body, cut) = clip(&t.body, BODY_MAX, o.full);
        m.insert("body".into(), json!(body));
        if cut > 0 {
            m.insert("body_chars".into(), json!(t.body.chars().count()));
        }
    }
    if o.full {
        if let Some(orig) = &t.original {
            m.insert("original".into(), json!({ "author": orig.author, "title": orig.title, "body": safe::clean(&orig.body) }));
        }
    }
    if !t.checklist.is_empty() {
        m.insert("checklist".into(), items_json(&t.checklist, o.full));
    }
    if !t.acceptance.is_empty() {
        m.insert("criteria".into(), items_json(&t.acceptance, o.full));
    }
    if !t.attachments.is_empty() {
        m.insert(
            "attachments".into(),
            json!(t
                .attachments
                .iter()
                .map(|a| json!({ "name": a.name, "size": a.size, "path": a.path }))
                .collect::<Vec<_>>()),
        );
    }
    let total = t.comments.len();
    if total > 0 {
        let shown = total.min(o.comments);
        m.insert(
            "comments".into(),
            json!(t.comments[total - shown..]
                .iter()
                .map(|c| {
                    let (body, cut) = clip(&c.body, COMMENT_MAX, o.full);
                    let mut v =
                        json!({ "id": c.id, "by": c.author, "at": iso(c.created), "body": body });
                    if cut > 0 {
                        v["body_chars"] = json!(c.body.chars().count());
                    }
                    v
                })
                .collect::<Vec<_>>()),
        );
        if shown < total {
            m.insert("comments_total".into(), json!(total));
        }
    }
    if o.activity || o.full {
        m.insert(
            "activity".into(),
            json!(t
                .activity
                .iter()
                .map(|a| {
                    let (kind, detail) = wipe_core::events::describe(&a.kind, &a.detail);
                    json!({ "at": iso(a.ts), "by": a.actor, "event": kind, "detail": safe::line(&detail) })
                })
                .collect::<Vec<_>>()),
        );
    }
    if o.commits && !commits.is_empty() {
        let cap = if o.full { usize::MAX } else { COMMITS_DEFAULT };
        m.insert(
            "commits".into(),
            json!(commits
                .iter()
                .take(cap)
                .map(|c| json!({ "hash": c.short, "subject": c.subject, "author": c.author_name, "date": c.date }))
                .collect::<Vec<_>>()),
        );
    }
    Value::Object(m)
}

/// The comment thread alone (`--comments-only`, `wipe comment list`).
pub fn print_comments(t: &Ticket, o: &Opts) {
    let total = t.comments.len();
    if total == 0 {
        println!("{} has no comments", t.id);
        return;
    }
    let shown = total.min(o.comments);
    if shown < total {
        println!(
            "{}",
            dim(&format!(
                "{total} comments, newest {shown} (`--comments all`):"
            ))
        );
    }
    for c in &t.comments[total - shown..] {
        println!(
            "{} {} {}",
            c.id,
            safe::line(&c.author),
            dim(&when(c.created))
        );
        let (body, cut) = clip(&c.body, COMMENT_MAX, o.full);
        for l in body.lines() {
            println!("  {l}");
        }
        if cut > 0 {
            println!("  {}", dim(&format!("… {cut} more chars (--full)")));
        }
    }
}
