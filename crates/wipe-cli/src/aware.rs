//! Board updates appended to every command: after any wipe command, a few lines
//! on stderr say what changed on the board since this identity's previous
//! command - tickets filed, its tickets moved or commented on, mentions, forum
//! replies - each reported once (see [`wipe_core::aware`]). This is how an agent
//! that never looks at the board still notices work aimed at it. stderr keeps
//! `--json` stdout a single object and pipes clean; harnesses show both.

use owo_colors::{OwoColorize, Stream};
use wipe_core::aware::{self, Update};
use wipe_core::model::Board;
use wipe_core::Store;

use crate::args::{Command, TicketCmd};
use crate::identity;
use crate::safe;

/// Lines printed before pointing at `wipe inbox` for the rest.
const MAX_LINES: usize = 8;

/// What a command does with the updates.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// Print them after the command's own output.
    Report,
    /// Advance the snapshot silently: the command showed the changes itself.
    Absorb,
    /// Leave the snapshot alone (setup, long-running, or reports them inline).
    Skip,
}

/// How `command` takes part in awareness.
pub fn mode(command: &Command) -> Mode {
    match command {
        Command::Inbox(_) | Command::Watch(_) => Mode::Absorb,
        Command::Init(_)
        | Command::Onboard(_)
        | Command::Identity(_)
        | Command::Serve(_)
        | Command::Tray(_)
        | Command::Update(_)
        | Command::Status(_)
        | Command::Completions { .. } => Mode::Skip,
        _ => Mode::Report,
    }
}

/// Whether a command hands in or finishes a ticket: the moment an agent picks
/// its next task, so it is reminded of the other work assigned to it.
#[derive(Clone, PartialEq)]
pub enum Finish {
    No,
    Yes,
    /// A move: finishing only when the target is the done or review list.
    MoveTo(String),
}

/// How `command` finishes a ticket, if it does.
pub fn finish(command: &Command) -> Finish {
    match command {
        Command::Ticket(TicketCmd::Close { .. } | TicketCmd::Submit(_) | TicketCmd::Approve(_)) => {
            Finish::Yes
        }
        Command::Ticket(TicketCmd::Move { list, to, .. }) => list
            .clone()
            .or_else(|| to.clone())
            .map(Finish::MoveTo)
            .unwrap_or(Finish::No),
        _ => Finish::No,
    }
}

/// Whether awareness is switched off (`WIPE_NO_UPDATES=1`).
fn disabled() -> bool {
    std::env::var_os("WIPE_NO_UPDATES").is_some_and(|v| !v.is_empty() && v != "0")
}

/// The updates for `identity` on `store`, as display lines (most important
/// first). Advances the identity's snapshot.
pub fn catch_up_lines(store: &Store, identity: &str) -> Vec<String> {
    if disabled() {
        return Vec::new();
    }
    match aware::catch_up(store, identity) {
        Ok(u) if !u.is_empty() => lines(store, identity, &u),
        _ => Vec::new(),
    }
}

/// Run after a command: report (or absorb) the updates for the acting identity
/// on the board in the current directory, then - when the command `finished` a
/// ticket - what else is still assigned to it. Best-effort, silent on failure.
pub fn after_command(mode: Mode, finished: Finish, actor: Option<&str>) {
    if mode == Mode::Skip || disabled() {
        return;
    }
    let Some(who) = identity::resolve_opt(actor) else {
        return;
    };
    let Ok(store) = Store::discover(".") else {
        return;
    };
    let lines = catch_up_lines(&store, &who);
    if mode == Mode::Report && !lines.is_empty() {
        eprint!("{}", render(&lines));
    }
    if finished != Finish::No {
        if let Some(next) = next_up(&store, &who, &finished) {
            eprintln!("{next}");
        }
    }
}

/// Tickets named in the "next up" reminder before pointing at search.
const NEXT_MAX: usize = 3;

/// "Still assigned to you: ..." - the open tickets assigned to `who`, nearest
/// to done first (in progress before todo before backlog). `None` when there
/// are none, or when a move did not go to the done / review list.
fn next_up(store: &Store, who: &str, finished: &Finish) -> Option<String> {
    let board = store.load_board().ok()?;
    let settings = store.load_settings().unwrap_or_default();
    let (review, done, _) = wipe_core::ops::workflow_lists(&board, &settings);
    let closed = |l: &str| Some(l) == done.as_deref() || Some(l) == review.as_deref();
    if let Finish::MoveTo(target) = finished {
        if !closed(target) {
            return None;
        }
    }
    let mut open: Vec<(String, String, String)> = Vec::new();
    for l in board.lists.iter().rev().filter(|l| !closed(&l.id)) {
        for id in &l.cards {
            let Ok(t) = store.load_ticket(id) else {
                continue;
            };
            if t.assignees.iter().any(|a| a == who) {
                open.push((t.id, t.title, safe::line(&l.name)));
            }
        }
    }
    if open.is_empty() {
        return None;
    }
    let tag = "[wipe]".if_supports_color(Stream::Stderr, |t| t.yellow().to_string());
    let items: Vec<String> = open
        .iter()
        .take(NEXT_MAX)
        .map(|(id, title, list)| format!("{id} {} ({list})", quote(title, 50)))
        .collect();
    let more = open.len().saturating_sub(NEXT_MAX);
    Some(format!(
        "{tag} still assigned to you: {}{}",
        items.join(", "),
        if more > 0 {
            format!(" +{more} more: wipe search @me is:open")
        } else {
            String::new()
        }
    ))
}

/// The stderr block: one tagged line for a single update, else a header and a
/// bulleted list capped at [`MAX_LINES`].
pub fn render(lines: &[String]) -> String {
    let tag = "[wipe]".if_supports_color(Stream::Stderr, |t| t.yellow().to_string());
    if let [one] = lines {
        return format!("{tag} board update: {one}\n");
    }
    let mut s = format!(
        "{tag} {} board updates since your last wipe command:\n",
        lines.len()
    );
    for l in lines.iter().take(MAX_LINES) {
        s.push_str(&format!("  - {l}\n"));
    }
    if lines.len() > MAX_LINES {
        s.push_str(&format!(
            "  - … {} more: wipe inbox\n",
            lines.len() - MAX_LINES
        ));
    }
    s
}

/// `"text"`, cleaned to one line and clipped.
fn quote(s: &str, max: usize) -> String {
    let (t, cut) = safe::clip(&safe::line(s), max);
    format!("\"{t}{}\"", if cut > 0 { "…" } else { "" })
}

fn by(actor: &str) -> String {
    if actor.is_empty() {
        String::new()
    } else {
        format!(" by {}", safe::line(actor))
    }
}

/// Distinct actors, in order, joined.
fn actors<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for n in names {
        if !n.is_empty() && !seen.contains(&n) {
            seen.push(n);
        }
    }
    seen.iter()
        .map(|n| safe::line(n))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The value for `key` in an insertion-ordered list of groups.
fn group<K: PartialEq, V>(groups: &mut Vec<(K, Vec<V>)>, key: K) -> &mut Vec<V> {
    let i = match groups.iter().position(|(k, _)| *k == key) {
        Some(i) => i,
        None => {
            groups.push((key, Vec::new()));
            groups.len() - 1
        }
    };
    &mut groups[i].1
}

/// Turn updates into display lines, grouped and ordered by how much they ask of
/// the reader: comments (mentions first), assignments, moves, new tickets, other
/// edits, forum replies.
pub fn lines(store: &Store, me: &str, updates: &[Update]) -> Vec<String> {
    let board = store.load_board().ok();
    let list = |id: &Option<String>| list_name(board.as_ref(), id.as_deref());
    let ticket = |u: &Update| format!("{} {}", u.event.id, quote(&u.event.title, 50));
    let yours = |u: &Update| {
        if u.why == "assigned" {
            "your ticket "
        } else {
            ""
        }
    };

    let mut assignment = Vec::new();
    let mut comments: Vec<(String, Vec<&Update>)> = Vec::new();
    let mut moves: Vec<((String, String), Vec<&Update>)> = Vec::new();
    let mut created: Vec<(String, Vec<&Update>)> = Vec::new();
    let mut edits: Vec<(String, Vec<&Update>)> = Vec::new();
    let mut posts: Vec<(String, Vec<&Update>)> = Vec::new();

    for u in updates {
        let e = &u.event;
        match e.event {
            "created" if u.why == "assigned" => assignment.push(format!(
                "new ticket {} in {}, assigned to you{}",
                ticket(u),
                list(&e.list),
                by(&e.by)
            )),
            "created" => group(&mut created, list(&e.list)).push(u),
            "assigned" if e.detail == format!("+{me}") => {
                assignment.push(format!("{} assigned to you{}", ticket(u), by(&e.by)))
            }
            "assigned" if e.detail == format!("-{me}") => {
                assignment.push(format!("{} unassigned from you{}", ticket(u), by(&e.by)))
            }
            "deleted" => assignment.push(format!("{}{} was deleted", yours(u), ticket(u))),
            "commented" => group(&mut comments, e.id.clone()).push(u),
            "moved" => group(&mut moves, (list(&e.from), list(&e.list))).push(u),
            "posted" => group(&mut posts, u.thread.clone().unwrap_or_else(|| e.id.clone())).push(u),
            _ => group(&mut edits, e.id.clone()).push(u),
        }
    }

    // Comments first - mentions ahead - since they usually ask for something on
    // the work in hand; then assignments.
    comments.sort_by_key(|(_, cs)| !cs.iter().any(|u| aware::mentions(&u.event.detail, me)));
    let mut out = Vec::new();
    for (_, cs) in &comments {
        let last = cs[cs.len() - 1];
        let mention = cs.iter().any(|u| aware::mentions(&u.event.detail, me));
        out.push(if cs.len() == 1 {
            format!(
                "{} {} {}{}: {}",
                safe::line(&last.event.by),
                if mention {
                    "mentioned you on"
                } else {
                    "commented on"
                },
                yours(last),
                ticket(last),
                quote(&last.event.detail, 100)
            )
        } else {
            format!(
                "{} new comments by {} on {}{}{}, latest: {}",
                cs.len(),
                actors(cs.iter().map(|u| u.event.by.as_str())),
                yours(last),
                ticket(last),
                if mention { ", mentioning you" } else { "" },
                quote(&last.event.detail, 100)
            )
        });
    }
    out.extend(assignment);
    for ((from, to), ms) in &moves {
        let route = if from.is_empty() {
            format!("to {to}")
        } else {
            format!("{from} → {to}")
        };
        let who = actors(ms.iter().map(|u| u.event.by.as_str()));
        let who = if who.is_empty() {
            String::new()
        } else {
            format!(" by {who}")
        };
        out.push(if let [one] = ms.as_slice() {
            format!("{}{} moved {route}{who}", yours(one), ticket(one))
        } else {
            let ids: Vec<&str> = ms.iter().map(|u| u.event.id.as_str()).collect();
            format!(
                "{} {}tickets moved {route}{who}: {}",
                ms.len(),
                if ms.iter().all(|u| u.why == "assigned") {
                    "of your "
                } else {
                    ""
                },
                ids.join(", ")
            )
        });
    }
    for (l, ts) in &created {
        let who = actors(ts.iter().map(|u| u.event.by.as_str()));
        out.push(if let [one] = ts.as_slice() {
            format!("new ticket in {l}{}: {}", by(&one.event.by), ticket(one))
        } else if ts.len() <= 3 {
            let items: Vec<String> = ts.iter().map(|u| ticket(u)).collect();
            format!(
                "{} new tickets in {l} by {who}: {}",
                ts.len(),
                items.join(", ")
            )
        } else {
            let ids: Vec<&str> = ts.iter().take(6).map(|u| u.event.id.as_str()).collect();
            let more = ts.len().saturating_sub(6);
            format!(
                "{} new tickets in {l} by {who}: {}{}",
                ts.len(),
                ids.join(", "),
                if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                }
            )
        });
    }
    for (_, es) in &edits {
        let first = es[0];
        let what: Vec<String> = es
            .iter()
            .map(|u| safe::line(&u.event.detail))
            .filter(|d| !d.is_empty())
            .collect();
        let (what, cut) = safe::clip(&what.join(", "), 80);
        out.push(format!(
            "{}{} changed{}: {what}{}",
            yours(first),
            ticket(first),
            match actors(es.iter().map(|u| u.event.by.as_str())) {
                w if w.is_empty() => String::new(),
                w => format!(" by {w}"),
            },
            if cut > 0 { "…" } else { "" }
        ));
    }
    for (thread, ps) in &posts {
        let last = ps[ps.len() - 1];
        let title = quote(&last.event.title, 50);
        let mention = ps.iter().any(|u| aware::mentions(&u.event.detail, me));
        out.push(if let [one] = ps.as_slice() {
            let verb = if mention {
                "mentioned you in"
            } else if one.event.id == *thread {
                "started"
            } else {
                "replied in"
            };
            format!(
                "{} {verb} {thread} {title}: {}",
                safe::line(&one.event.by),
                quote(&one.event.detail, 100)
            )
        } else {
            format!(
                "{} new posts by {} in {thread} {title}{}, latest: {}",
                ps.len(),
                actors(ps.iter().map(|u| u.event.by.as_str())),
                if mention { ", mentioning you" } else { "" },
                quote(&last.event.detail, 100)
            )
        });
    }
    out
}

fn list_name(board: Option<&Board>, id: Option<&str>) -> String {
    let Some(id) = id else {
        return String::new();
    };
    board
        .and_then(|b| b.list(id))
        .map(|l| safe::line(&l.name))
        .unwrap_or_else(|| safe::line(id))
}
