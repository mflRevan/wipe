//! `wipe watch`: wait for board changes and report each as one line.
//!
//! Built for agent harnesses, which all share one primitive - a shell command,
//! its stdout and its exit code:
//!
//! - streamed (the default), every change is one flushed line, so a harness
//!   that turns output lines into notifications (e.g. a monitor tool) gets one
//!   per change;
//! - with `--until`, the first matching change is printed and the command exits
//!   0 - a background task's completion *is* the notification, and a blocking
//!   call (`--timeout 10m`, exit 124 when nothing happened) works everywhere.
//!
//! Your own changes are never reported, nothing is printed for the current
//! state (only later changes), and each line carries what is needed to act on
//! it without another lookup: id, what happened, by whom, the ticket's title,
//! and the comment text for comments.

use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::json;

use wipe_core::events::{Event, Watcher, KINDS};
use wipe_core::Store;

use crate::args::WatchArgs;
use crate::identity;
use crate::output::Out;
use crate::safe;
use crate::view;

/// What a watch listens to.
#[derive(Default)]
struct Filter {
    tickets: Vec<String>,
    lists: Vec<String>,
    threads: Vec<String>,
    forum: bool,
    kinds: Vec<String>,
    from: Option<String>,
    to: Option<String>,
    by: Option<String>,
    me: Option<String>,
}

impl Filter {
    fn wants(&self, e: &Event) -> bool {
        if self.me.as_deref().is_some_and(|me| e.by == me) {
            return false;
        }
        if !self.kinds.is_empty() && !self.kinds.iter().any(|k| k == e.event) {
            return false;
        }
        if let Some(by) = &self.by {
            if !e.by.to_lowercase().contains(by.as_str()) {
                return false;
            }
        }
        if self.from.is_some() || self.to.is_some() {
            if e.event != "moved" {
                return false;
            }
            if self
                .from
                .as_ref()
                .is_some_and(|f| e.from.as_ref() != Some(f))
            {
                return false;
            }
            if self.to.as_ref().is_some_and(|t| e.list.as_ref() != Some(t)) {
                return false;
            }
        }
        let targeted = !self.tickets.is_empty()
            || !self.lists.is_empty()
            || !self.threads.is_empty()
            || self.forum;
        if !targeted {
            return true;
        }
        if e.event == "posted" {
            let thread = e.id.split('.').next().unwrap_or_default();
            return self.forum || self.threads.iter().any(|t| t == thread);
        }
        self.tickets.contains(&e.id)
            || e.list.as_ref().is_some_and(|l| self.lists.contains(l))
            || e.from.as_ref().is_some_and(|l| self.lists.contains(l))
    }
}

fn duration(raw: &str) -> Result<Duration> {
    let s = raw.trim();
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (n, unit) = s.split_at(split);
    let n: u64 = n
        .parse()
        .map_err(|_| anyhow::anyhow!("cannot read `{raw}` as a duration - e.g. 90s, 10m, 2h"))?;
    Ok(Duration::from_secs(match unit {
        "" | "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        _ => bail!("cannot read `{raw}` as a duration - e.g. 90s, 10m, 2h"),
    }))
}

/// One event as a text line: `T-02A moved review → done · ada · Title` /
/// `T-02A commented · ada · Title: "text"`.
pub fn line(e: &Event) -> String {
    let title = view::title(&e.title);
    let by = if e.by.is_empty() {
        String::new()
    } else {
        format!(" · {}", safe::line(&e.by))
    };
    let detail = safe::line(&e.detail);
    match e.event {
        "moved" => format!(
            "{} moved {} → {}{by} · {title}",
            e.id,
            e.from.as_deref().unwrap_or("?"),
            e.list.as_deref().unwrap_or("?")
        ),
        "commented" | "posted" => format!("{} {}{by} · {title}: \"{detail}\"", e.id, e.event),
        "deleted" => format!(
            "{} deleted from {}{by} · {title}",
            e.id,
            e.from.as_deref().unwrap_or("?")
        ),
        "created" => format!(
            "{} created in {}{by} · {title}",
            e.id,
            e.list.as_deref().unwrap_or("?")
        ),
        _ if detail.is_empty() => format!("{} {}{by} · {title}", e.id, e.event),
        _ => format!("{} {} {detail}{by} · {title}", e.id, e.event),
    }
}

fn emit(out: &Out, e: &Event) -> std::io::Result<()> {
    let mut o = std::io::stdout().lock();
    if out.json {
        let mut v = serde_json::to_value(e).unwrap_or_default();
        v["title"] = json!(view::title(&e.title));
        v["detail"] = json!(safe::line(&e.detail));
        if e.detail.is_empty() {
            v.as_object_mut().map(|m| m.remove("detail"));
        }
        v["ts"] = json!(crate::show::iso(e.ts));
        writeln!(o, "{v}")?;
    } else {
        writeln!(o, "{}", line(e))?;
    }
    o.flush()
}

/// `wipe watch`
pub fn run(out: &Out, a: WatchArgs) -> Result<()> {
    let store = Store::discover(".")?;
    let board = store.load_board()?;
    let mut f = Filter {
        kinds: a
            .on
            .iter()
            .map(|k| k.trim().to_lowercase())
            .filter(|k| !k.is_empty())
            .collect(),
        by: a.by.map(|b| b.to_lowercase()),
        ..Default::default()
    };
    for k in &f.kinds {
        if !KINDS.contains(&k.as_str()) {
            bail!(
                "unknown event `{k}` for --on - one of: {}",
                KINDS.join(", ")
            );
        }
    }
    let list_id = |raw: &str| -> Option<String> {
        let low = raw.to_lowercase();
        board
            .lists
            .iter()
            .find(|l| l.id == low || l.name.to_lowercase() == low)
            .map(|l| l.id.clone())
    };
    for (flag, val) in [("--from", &a.from), ("--to", &a.to)] {
        if let Some(v) = val {
            let Some(id) = list_id(v) else {
                bail!("{flag}: no list `{v}` - lists: {}", lists_of(&board));
            };
            if flag == "--from" {
                f.from = Some(id);
            } else {
                f.to = Some(id);
            }
        }
    }
    for t in &a.targets {
        let low = t.to_lowercase();
        if low == "forum" {
            f.forum = true;
        } else if low.starts_with("f-") && low[2..].chars().all(|c| c.is_ascii_digit()) {
            f.threads.push(t.to_uppercase());
        } else if let Some(id) = t
            .strip_prefix("list:")
            .and_then(list_id)
            .or_else(|| list_id(t))
        {
            f.lists.push(id);
        } else if let Ok(id) = store.resolve_ticket_id(t) {
            f.tickets.push(id);
        } else {
            bail!(
                "nothing to watch called `{t}` - a ticket (T-02A), a list ({}), a thread (F-3), or `forum`",
                lists_of(&board)
            );
        }
    }
    if !a.include_self {
        f.me = identity::resolve_opt(None);
    }
    let timeout = a.timeout.as_deref().map(duration).transpose()?;
    let stop_after = if a.until { Some(1) } else { a.count };
    let interval = Duration::from_millis(a.interval.clamp(100, 60_000));

    let scope = if a.targets.is_empty() {
        "the board".to_string()
    } else {
        a.targets.join(", ")
    };
    let what = if f.kinds.is_empty() {
        String::new()
    } else {
        format!(" ({})", f.kinds.join(","))
    };
    let mine = match (&f.me, a.include_self) {
        (Some(me), _) => format!("not yours ({me})"),
        (None, true) => "including yours".into(),
        (None, false) => "no identity, so yours too".into(),
    };
    let ending = match (stop_after, timeout) {
        (Some(1), Some(t)) => format!("exits on the first, or after {}s", t.as_secs()),
        (Some(1), None) => "exits on the first".into(),
        (Some(n), _) => format!("exits after {n}"),
        (None, Some(t)) => format!("stops after {}s", t.as_secs()),
        (None, None) => "until stopped".into(),
    };
    // Snapshot first, then announce: a change landing right after the banner
    // is always reported.
    let mut watcher = Watcher::new(store)?;
    eprintln!("watching {scope}{what} - changes {mine}, {ending}");
    let start = Instant::now();
    let mut seen = 0usize;
    loop {
        std::thread::sleep(interval);
        for e in watcher.poll() {
            if !f.wants(&e) {
                continue;
            }
            if emit(out, &e).is_err() {
                return Ok(()); // the reader went away
            }
            seen += 1;
            if stop_after.is_some_and(|n| seen >= n) {
                return Ok(());
            }
        }
        if timeout.is_some_and(|t| start.elapsed() >= t) {
            eprintln!(
                "no matching change within {}s",
                timeout.unwrap_or_default().as_secs()
            );
            std::process::exit(if seen > 0 { 0 } else { 124 });
        }
    }
}

fn lists_of(board: &wipe_core::model::Board) -> String {
    board
        .lists
        .iter()
        .map(|l| l.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn ev(
        event: &'static str,
        id: &str,
        by: &str,
        from: Option<&str>,
        list: Option<&str>,
    ) -> Event {
        Event {
            ts: Utc::now(),
            event,
            id: id.into(),
            title: "Fix the foam".into(),
            by: by.into(),
            list: list.map(Into::into),
            from: from.map(Into::into),
            detail: String::new(),
        }
    }

    #[test]
    fn filters_and_lines() {
        let f = Filter {
            tickets: vec!["T-02A".into()],
            lists: vec!["review".into()],
            me: Some("claude".into()),
            ..Default::default()
        };
        assert!(f.wants(&ev("commented", "T-02A", "ada", None, Some("todo"))));
        assert!(
            !f.wants(&ev("commented", "T-02A", "claude", None, Some("todo"))),
            "own change"
        );
        assert!(
            f.wants(&ev("moved", "T-001", "ada", Some("review"), Some("done"))),
            "left a watched list"
        );
        assert!(!f.wants(&ev("moved", "T-001", "ada", Some("todo"), Some("doing"))));

        let out_of_review = Filter {
            from: Some("review".into()),
            ..Default::default()
        };
        assert!(out_of_review.wants(&ev("moved", "T-1", "ada", Some("review"), Some("todo"))));
        assert!(!out_of_review.wants(&ev("moved", "T-1", "ada", Some("todo"), Some("review"))));
        assert!(!out_of_review.wants(&ev("commented", "T-1", "ada", None, Some("review"))));

        let forum = Filter {
            threads: vec!["F-3".into()],
            ..Default::default()
        };
        assert!(forum.wants(&ev("posted", "F-3.2", "ada", None, None)));
        assert!(!forum.wants(&ev("posted", "F-30", "ada", None, None)));
        assert!(!forum.wants(&ev("commented", "T-1", "ada", None, None)));

        // Each line carries what to act on.
        let mut e = ev("moved", "T-02A", "ada", Some("review"), Some("done"));
        assert_eq!(line(&e), "T-02A moved review → done · ada · Fix the foam");
        e.event = "commented";
        e.detail = "please re-check\nthe edges".into();
        assert_eq!(
            line(&e),
            "T-02A commented · ada · Fix the foam: \"please re-check the edges\""
        );
        assert_eq!(duration("10m").unwrap(), Duration::from_secs(600));
        assert!(duration("soon").is_err());
    }
}
