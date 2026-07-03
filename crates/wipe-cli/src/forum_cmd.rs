//! `wipe forum ...` - post to and search the project's git-tracked forum.

use std::collections::HashSet;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

use wipe_core::forum::{self, NewReply, NewThread, PostView, SearchQuery};
use wipe_core::model::{Attachment, Post};
use wipe_core::Store;

use crate::args::*;
use crate::commands::guess_mime;
use crate::identity;
use crate::output::{dim, id_style, Out};

fn store() -> Result<Store> {
    Store::discover(".").map_err(Into::into)
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("model is serializable")
}

/// Read and stage each attachment path into an [`Attachment`] (dedup vs repo blobs
/// or copy into `.wipe/media/`), respecting the board's size limit.
fn stage_attachments(store: &Store, paths: &[PathBuf]) -> Result<Vec<Attachment>> {
    let limit = store.load_settings()?.max_attachment_mb * 1024 * 1024;
    let mut out = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("invalid file name: {}", path.display()))?
            .to_string();
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if bytes.len() as u64 > limit {
            bail!(
                "{name} is {:.1} MB, over the {} MB attachment limit",
                bytes.len() as f64 / 1_048_576.0,
                limit / 1024 / 1024
            );
        }
        out.push(wipe_core::ops::stage_media(
            store,
            &name,
            &bytes,
            guess_mime(&name),
        )?);
    }
    Ok(out)
}

/// First non-empty line of a body, trimmed to a readable length.
fn snippet(body: &str) -> String {
    let line = body
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() > 100 {
        let s: String = line.chars().take(97).collect();
        format!("{s}...")
    } else {
        line.to_string()
    }
}

pub fn run(out: &Out, cmd: ForumCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        ForumCmd::Post(a) => {
            let attachments = stage_attachments(&s, &a.attach)?;
            let author = identity::resolve(a.author);
            let spec = NewThread {
                title: a.title,
                body: a.body.unwrap_or_default(),
                labels: a.labels,
                refs: a.refs,
                attachments,
            };
            let t = forum::create_thread(&s, spec, &author, Utc::now())?;
            out.ok(
                format!("posted {} - {}", t.id, t.title),
                json!({ "ok": true, "id": t.id, "title": t.title, "author": author }),
            );
        }

        ForumCmd::Reply(a) => {
            let attachments = stage_attachments(&s, &a.attach)?;
            let author = identity::resolve(a.author);
            let parent = a.id.clone();
            let spec = NewReply {
                body: a.body,
                labels: a.labels,
                refs: a.refs,
                attachments,
            };
            let id = forum::reply(&s, &parent, spec, &author, Utc::now())?;
            out.ok(
                format!("replied {id} (to {parent})"),
                json!({ "ok": true, "id": id, "parent": parent, "author": author }),
            );
        }

        ForumCmd::Show { id, depth } => {
            let thread = forum::get_thread(&s, &id)?;
            let root = thread
                .root
                .find(&id)
                .ok_or_else(|| anyhow!("forum post `{id}` not found"))?;
            if out.json {
                out.json_value(
                    &json!({ "thread": thread.id, "title": thread.title, "post": to_value(root) }),
                );
            } else {
                if root.id == thread.id {
                    println!("{}  {}", id_style(&thread.id), thread.title);
                }
                render(root, 0, depth);
            }
        }

        ForumCmd::List {
            label,
            author,
            limit,
        } => {
            let all = forum::index(&s)?;
            let mut roots: Vec<&PostView> = all
                .iter()
                .filter(|p| p.depth == 0)
                .filter(|r| label.as_ref().map_or(true, |l| r.labels.contains(l)))
                .filter(|r| {
                    author.as_ref().map_or(true, |a| {
                        r.author.to_lowercase().contains(&a.to_lowercase())
                    })
                })
                .collect();
            if let Some(n) = limit {
                roots.truncate(n);
            }
            let total_of = |tid: &str| all.iter().filter(|p| p.thread_id == tid).count();
            if out.json {
                let arr: Vec<Value> = roots
                    .iter()
                    .map(|r| {
                        json!({
                            "id": r.thread_id,
                            "title": r.thread_title,
                            "author": r.author,
                            "labels": r.labels,
                            "posts": total_of(&r.thread_id),
                            "created": r.created,
                        })
                    })
                    .collect();
                out.json_value(&json!(arr));
            } else if roots.is_empty() {
                out.line("no threads yet - start one with `wipe forum post`");
            } else {
                for r in &roots {
                    println!(
                        "{}  {}  {}",
                        id_style(&r.thread_id),
                        r.thread_title,
                        dim(&format!(
                            "({} posts · {})",
                            total_of(&r.thread_id),
                            r.author
                        ))
                    );
                }
            }
        }

        ForumCmd::Search(a) => {
            // A blank/whitespace-only pattern means "no pattern" (match all),
            // matching the daemon's behavior.
            let pattern = match a.pattern.as_deref() {
                Some(p) if !p.trim().is_empty() => {
                    Some(forum::compile_pattern(p, !a.case_sensitive)?)
                }
                _ => None,
            };
            let q = SearchQuery {
                pattern,
                author: a.author,
                labels: a.labels,
                scope: a.scope,
                max_depth: a.depth,
                titles_only: a.titles,
                limit: a.limit,
            };
            let hits = forum::search(&s, &q)?;
            if out.json {
                out.json_value(&to_value(&hits));
            } else if hits.is_empty() {
                out.line("no matching posts");
            } else {
                for h in &hits {
                    let labels = if h.labels.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", dim(&format!("[{}]", h.labels.join(","))))
                    };
                    println!(
                        "{}  {}{}  {}",
                        id_style(&h.id),
                        dim(&h.author),
                        labels,
                        snippet(&h.body)
                    );
                }
                println!(
                    "{}",
                    dim(&format!(
                        "{} match(es) · `wipe forum show <id>` to open, `wipe forum reply <id>` to respond",
                        hits.len()
                    ))
                );
            }
        }

        ForumCmd::Edit { id, body } => {
            forum::edit_post(&s, &id, &body, Utc::now())?;
            out.ok(format!("edited {id}"), json!({ "ok": true, "id": id }));
        }

        ForumCmd::Delete { id, yes } => {
            if !yes {
                bail!("refusing to delete {id} and its replies without --yes");
            }
            forum::delete_post(&s, &id, Utc::now())?;
            out.ok(
                format!("deleted {id} and its replies"),
                json!({ "ok": true, "id": id }),
            );
        }

        ForumCmd::Watch(a) => watch(&s, a)?,
    }
    Ok(())
}

/// Render a post and (bounded by `max_depth`) its reply subtree as an indented tree.
fn render(post: &Post, level: usize, max_depth: Option<usize>) {
    let pad = "  ".repeat(level);
    let labels = if post.labels.is_empty() {
        String::new()
    } else {
        format!(" {}", dim(&format!("[{}]", post.labels.join(","))))
    };
    println!(
        "{pad}{} {}{}",
        id_style(&post.id),
        dim(&post.author),
        labels
    );
    for line in post.body.lines() {
        println!("{pad}  {line}");
    }
    if max_depth.map_or(true, |m| level < m) {
        for r in &post.replies {
            render(r, level + 1, max_depth);
        }
    }
}

/// Stream new matching posts as newline-delimited JSON until interrupted.
fn watch(store: &Store, a: ForumWatchArgs) -> Result<()> {
    let pattern = match a.pattern.as_deref() {
        Some(p) if !p.trim().is_empty() => Some(forum::compile_pattern(p, true)?),
        _ => None,
    };
    let q = SearchQuery {
        pattern,
        author: a.author,
        labels: a.labels,
        scope: a.scope,
        ..Default::default()
    };
    let interval = Duration::from_millis(a.interval.max(50));

    // One initial snapshot seeds both `seen` (every id) and the optional replay
    // (its matching subset, oldest-first). Using a single snapshot closes the
    // window where a post created between two separate reads would be recorded as
    // seen yet never emitted.
    let mut seen: HashSet<String> = {
        let initial = forum::index(store)?;
        if a.replay {
            let mut matched: Vec<&PostView> =
                initial.iter().filter(|p| forum::matches(p, &q)).collect();
            matched.sort_by_key(|p| p.created);
            for p in matched {
                emit(p);
            }
        }
        initial.into_iter().map(|p| p.id).collect()
    };

    loop {
        std::thread::sleep(interval);
        let all = match forum::index(store) {
            Ok(a) => a,
            Err(_) => continue,
        };
        let mut fresh: Vec<&PostView> = all
            .iter()
            .filter(|p| !seen.contains(&p.id) && forum::matches(p, &q))
            .collect();
        fresh.sort_by_key(|p| p.created); // oldest-first
        for p in fresh {
            emit(p);
        }
        // Rebuild `seen` from the current ids each poll: bounded memory, and safe
        // because dotted ids are never reused (a deleted id can never return).
        seen = all.into_iter().map(|p| p.id).collect();
    }
}

/// Write one NDJSON event to stdout and flush (so listeners get it immediately).
fn emit(p: &PostView) {
    let event = json!({
        "id": p.id,
        "thread": p.thread_id,
        "title": p.thread_title,
        "author": p.author,
        "labels": p.labels,
        "depth": p.depth,
        "body": p.body,
        "created": p.created,
    });
    let mut lock = std::io::stdout().lock();
    let _ = writeln!(lock, "{event}");
    let _ = lock.flush();
}
