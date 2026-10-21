//! Ticket and forum search with a small, GitHub-like query language.
//!
//! `wipe search` hands the whole query to [`parse`] and gets back ranked
//! [`Hit`]s, each with the one snippet that shows *why* it matched - so a caller
//! never has to page through bodies and comments itself.
//!
//! Syntax: free terms (all must match, case-insensitive), `"quoted phrases"`,
//! `-term` to exclude, ticket ids in any spelling (`T-2A`, `t2a`), and
//! qualifiers `label:`, `list:`, `assignee:` / `@who`, `author:`, `priority:`,
//! `is:open|done|blocked|ready|assigned|unassigned`. Qualifier values are
//! case-insensitive; repeat a qualifier to require several (`label:a label:b`).

use serde::Serialize;

use crate::error::Result;
use crate::model::{Board, Ticket};
use crate::{forum, ops, Store};

/// A parsed query.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Query {
    /// Words and phrases that must all appear (lower-cased).
    pub terms: Vec<String>,
    /// Words and phrases that must not appear.
    pub not: Vec<String>,
    /// Ticket ids named in the query (as typed).
    pub ids: Vec<String>,
    /// `label:` values - all required.
    pub labels: Vec<String>,
    /// `list:` values - any of them.
    pub lists: Vec<String>,
    /// `assignee:` / `@` values - all required (substring).
    pub assignees: Vec<String>,
    /// `author:` values - the creator (tickets) or poster (forum), substring.
    pub authors: Vec<String>,
    /// `priority:` values - any of them.
    pub priorities: Vec<String>,
    /// `is:` states - all required.
    pub states: Vec<String>,
}

impl Query {
    /// Whether the query has no criteria at all.
    pub fn is_empty(&self) -> bool {
        *self == Query::default()
    }

    /// Whether any criterion only applies to tickets (so forum posts can't match).
    fn ticket_only(&self) -> bool {
        !(self.ids.is_empty()
            && self.labels.is_empty()
            && self.lists.is_empty()
            && self.assignees.is_empty()
            && self.priorities.is_empty()
            && self.states.is_empty())
    }
}

/// `is:` states [`parse`] accepts.
pub const STATES: &[&str] = &["open", "done", "blocked", "ready", "assigned", "unassigned"];

fn tokens(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in q.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                if !quoted {
                    // keep the phrase marker so `-"a b"` and `label:"a b"` work
                    cur.push('\u{0}');
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .map(|t| t.replace('\u{0}', ""))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Parse a query string. Unknown `key:value` pairs are kept as plain terms, so
/// a search for `http://x` or `a:b` still works.
pub fn parse(q: &str) -> Query {
    let mut out = Query::default();
    for tok in tokens(q) {
        let (neg, tok) = match tok.strip_prefix('-') {
            Some(rest) if !rest.is_empty() => (true, rest.to_string()),
            _ => (false, tok),
        };
        let lower = tok.to_lowercase();
        if !neg {
            if let Some(who) = lower.strip_prefix('@').filter(|w| !w.is_empty()) {
                out.assignees.push(who.to_string());
                continue;
            }
            if let Some((key, val)) = lower.split_once(':') {
                let val = val.to_string();
                let slot = match key {
                    "label" | "labels" => Some(&mut out.labels),
                    "list" | "in" => Some(&mut out.lists),
                    "assignee" | "assigned" => Some(&mut out.assignees),
                    "author" | "by" => Some(&mut out.authors),
                    "priority" | "prio" => Some(&mut out.priorities),
                    "is" | "state" => Some(&mut out.states),
                    _ => None,
                };
                if let (Some(slot), false) = (slot, val.is_empty()) {
                    slot.push(val);
                    continue;
                }
            }
            // `T-2A`, `t2a`, `T-5` - but not words like `tea` or `tabs` that
            // happen to be `t` + hex letters: an id needs its dash or a digit.
            if crate::id::ticket_ref_digits(&tok).is_some()
                && tok.to_ascii_lowercase().starts_with('t')
                && tok.chars().count() <= 8
                && (tok.contains('-') || tok.chars().any(|c| c.is_ascii_digit()))
            {
                out.ids.push(tok);
                continue;
            }
        }
        if neg {
            out.not.push(lower);
        } else {
            out.terms.push(lower);
        }
    }
    out
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    /// `T-02A` or a forum post id `F-3.2`.
    pub id: String,
    /// Ticket title, or the post's thread title.
    pub title: String,
    /// The ticket's list (tickets only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    /// Relevance; higher is better.
    pub score: u32,
    /// Where the terms matched outside the title (`body`, `comment c-3 by ada`,
    /// `checklist`, `attachment`...), when they did not all match the title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched: Option<String>,
    /// The matching text around the first term, one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// A searchable field of a ticket: its label (for `matched`), weight, and text.
struct Field {
    name: String,
    weight: u32,
    text: String,
}

fn ticket_fields(t: &Ticket) -> Vec<Field> {
    let mut f = vec![
        Field {
            name: "title".into(),
            weight: 10,
            text: t.title.to_lowercase(),
        },
        Field {
            name: "labels".into(),
            weight: 8,
            text: t.labels.join(" ").to_lowercase(),
        },
        Field {
            name: "assignees".into(),
            weight: 6,
            text: t.assignees.join(" ").to_lowercase(),
        },
        Field {
            name: "body".into(),
            weight: 3,
            text: t.body.to_lowercase(),
        },
    ];
    for (name, items) in [("checklist", &t.checklist), ("criteria", &t.acceptance)] {
        for i in items {
            f.push(Field {
                name: format!("{name} {}", i.id),
                weight: 3,
                text: i.text.to_lowercase(),
            });
        }
    }
    for a in &t.attachments {
        f.push(Field {
            name: "attachment".into(),
            weight: 3,
            text: a.name.to_lowercase(),
        });
    }
    for c in &t.comments {
        f.push(Field {
            name: format!("comment {} by {}", c.id, c.author),
            weight: 2,
            text: c.body.to_lowercase(),
        });
    }
    if let Some(o) = &t.original {
        f.push(Field {
            name: "original".into(),
            weight: 1,
            text: format!("{} {}", o.title, o.body).to_lowercase(),
        });
    }
    f
}

/// The text around `needle`'s first occurrence in `hay` (already lower-cased),
/// as one line of at most ~`width` characters. Works on char boundaries.
pub fn around(original: &str, needle: &str, width: usize) -> String {
    let flat = original.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = flat.to_lowercase();
    // Lower-casing can change byte lengths; fall back to the start when the
    // byte offset does not map cleanly back to `flat`.
    let pos = lower
        .find(needle)
        .filter(|_| lower.len() == flat.len())
        .map(|b| flat[..b].chars().count())
        .unwrap_or(0);
    let chars: Vec<char> = flat.chars().collect();
    let start = pos.saturating_sub(width / 3);
    let end = (start + width).min(chars.len());
    let mut s: String = chars[start..end].iter().collect();
    if start > 0 {
        s = format!("…{}", s.trim_start());
    }
    if end < chars.len() {
        s = format!("{}…", s.trim_end());
    }
    s
}

/// Score `fields` against `terms`; `None` when some term matches nowhere.
fn score(fields: &[Field], terms: &[String]) -> Option<(u32, Option<usize>)> {
    let mut total = 0;
    let mut first_off_title: Option<usize> = None;
    for term in terms {
        let mut best: Option<(u32, usize)> = None;
        for (i, f) in fields.iter().enumerate() {
            if f.text.contains(term.as_str()) && best.is_none_or(|(w, _)| f.weight > w) {
                best = Some((f.weight, i));
            }
        }
        let (w, i) = best?;
        total += w;
        if i != 0 && first_off_title.is_none() {
            first_off_title = Some(i);
        }
    }
    Some((total, first_off_title))
}

fn ids_match(store: &Store, q: &Query, t: &Ticket) -> bool {
    q.ids
        .iter()
        .all(|raw| store.resolve_ticket_id(raw).is_ok_and(|id| id == t.id))
}

fn ticket_matches(
    store: &Store,
    q: &Query,
    t: &Ticket,
    list: &str,
    board: &Board,
    done: &str,
) -> bool {
    let lc = |v: &[String]| v.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
    let labels = lc(&t.labels);
    let assignees = lc(&t.assignees);
    if !q.labels.iter().all(|l| labels.contains(l)) {
        return false;
    }
    if !q
        .assignees
        .iter()
        .all(|a| assignees.iter().any(|x| x.contains(a.as_str())))
    {
        return false;
    }
    if !q.lists.is_empty() {
        let name = board
            .list(list)
            .map(|l| l.name.to_lowercase())
            .unwrap_or_default();
        if !q.lists.iter().any(|l| *l == list || *l == name) {
            return false;
        }
    }
    if !q.priorities.is_empty()
        && !t
            .priority
            .as_deref()
            .is_some_and(|p| q.priorities.contains(&p.to_lowercase()))
    {
        return false;
    }
    if !q.authors.is_empty() {
        let by = t.creator().unwrap_or_default().to_lowercase();
        if !q.authors.iter().all(|a| by.contains(a.as_str())) {
            return false;
        }
    }
    for st in &q.states {
        let blocked = || !ops::open_blockers(t, board, done).is_empty();
        let ok = match st.as_str() {
            "open" => list != done,
            "done" | "closed" => list == done,
            "blocked" => blocked(),
            "ready" => list != done && !blocked(),
            "assigned" => !t.assignees.is_empty(),
            "unassigned" => t.assignees.is_empty(),
            _ => true,
        };
        if !ok {
            return false;
        }
    }
    ids_match(store, q, t)
}

/// Search the board's tickets (in board order before ranking). `done` is the
/// done list's id, for `is:` states.
pub fn tickets(store: &Store, q: &Query, done: &str) -> Result<Vec<Hit>> {
    let (board, view) = ops::board_view(store)?;
    let mut hits = Vec::new();
    for (list, tickets) in &view {
        for t in tickets {
            if !ticket_matches(store, q, t, list, &board, done) {
                continue;
            }
            let fields = ticket_fields(t);
            let mut all = fields
                .iter()
                .map(|f| f.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            all.push_str(&format!("\n{}", t.id.to_lowercase()));
            if q.not.iter().any(|n| all.contains(n.as_str())) {
                continue;
            }
            let (mut s, off) = if q.terms.is_empty() {
                (0, None)
            } else {
                match score(&fields, &q.terms) {
                    Some(x) => x,
                    None => continue,
                }
            };
            if !q.ids.is_empty() {
                s += 100;
            }
            // All terms in the title outranks scattered matches.
            if !q.terms.is_empty() && q.terms.iter().all(|x| fields[0].text.contains(x.as_str())) {
                s += 5;
            }
            let (matched, snippet) = match off {
                Some(i) => {
                    let f = &fields[i];
                    let original = original_text(t, &f.name);
                    let term = q
                        .terms
                        .iter()
                        .find(|x| f.text.contains(x.as_str()))
                        .cloned()
                        .unwrap_or_default();
                    (Some(f.name.clone()), Some(around(&original, &term, 100)))
                }
                None => (None, None),
            };
            hits.push(Hit {
                id: t.id.clone(),
                title: t.title.clone(),
                list: Some(list.clone()),
                score: s,
                matched,
                snippet,
            });
        }
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.score));
    Ok(hits)
}

/// The un-lowered text of the field named `name` (for snippets).
fn original_text(t: &Ticket, name: &str) -> String {
    match name {
        "title" => t.title.clone(),
        "labels" => t.labels.join(" "),
        "assignees" => t.assignees.join(" "),
        "body" => t.body.clone(),
        "attachment" => t
            .attachments
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        "original" => t
            .original
            .as_ref()
            .map(|o| format!("{} {}", o.title, o.body))
            .unwrap_or_default(),
        n => {
            let mut parts = n.split(' ');
            let kind = parts.next().unwrap_or_default();
            let id = parts.next().unwrap_or_default();
            match kind {
                "checklist" => t
                    .checklist
                    .iter()
                    .find(|i| i.id == id)
                    .map(|i| i.text.clone()),
                "criteria" => t
                    .acceptance
                    .iter()
                    .find(|i| i.id == id)
                    .map(|i| i.text.clone()),
                "comment" => t
                    .comments
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.body.clone()),
                _ => None,
            }
            .unwrap_or_default()
        }
    }
}

/// Search forum posts (thread titles and bodies). Ticket-only qualifiers
/// (`label:`... apply to tickets) make this return nothing.
pub fn posts(store: &Store, q: &Query) -> Result<Vec<Hit>> {
    if q.ticket_only() || (q.terms.is_empty() && q.authors.is_empty()) {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    for p in forum::index(store)? {
        let author = p.author.to_lowercase();
        if !q.authors.iter().all(|a| author.contains(a.as_str())) {
            continue;
        }
        let fields = vec![
            Field {
                name: "title".into(),
                weight: if p.depth == 0 { 10 } else { 1 },
                text: p.thread_title.to_lowercase(),
            },
            Field {
                name: format!("post by {}", p.author),
                weight: 3,
                text: p.body.to_lowercase(),
            },
        ];
        let all = format!("{}\n{}", fields[0].text, fields[1].text);
        if q.not.iter().any(|n| all.contains(n.as_str())) {
            continue;
        }
        let Some((s, off)) = (if q.terms.is_empty() {
            Some((1, None))
        } else {
            score(&fields, &q.terms)
        }) else {
            continue;
        };
        let term = q
            .terms
            .iter()
            .find(|x| fields[1].text.contains(x.as_str()))
            .cloned();
        hits.push(Hit {
            id: p.id.clone(),
            title: p.thread_title.clone(),
            list: None,
            score: s,
            matched: off.map(|_| fields[1].name.clone()),
            snippet: term.map(|t| around(&p.body, &t, 100)),
        });
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.score));
    // One hit per thread: its best-scoring post.
    let mut seen = std::collections::HashSet::new();
    hits.retain(|h| seen.insert(h.id.split('.').next().unwrap_or_default().to_string()));
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::NewTicket;
    use chrono::Utc;

    #[test]
    fn parses_terms_phrases_negation_ids_and_qualifiers() {
        let q = parse(
            r#"foam "storm waves" -wip T-2a label:Ocean @Claude is:open list:review prio:high author:bil http://x a:b"#,
        );
        assert_eq!(q.terms, ["foam", "storm waves", "http://x", "a:b"]);
        assert_eq!(q.not, ["wip"]);
        assert_eq!(q.ids, ["T-2a"]);
        assert_eq!(q.labels, ["ocean"]);
        assert_eq!(q.assignees, ["claude"]);
        assert_eq!(q.states, ["open"]);
        assert_eq!(q.lists, ["review"]);
        assert_eq!(q.priorities, ["high"]);
        assert_eq!(q.authors, ["bil"]);
        assert!(parse("  ").is_empty());
        assert_eq!(parse("-").terms, ["-"]);
        assert_eq!(
            parse("tea").terms,
            ["tea"],
            "words starting with t are not ids"
        );
        // Snippets cut on char boundaries, whatever the script.
        let got = around(
            "Ünïcødé text with ß and the needle inside ✓ more words follow",
            "needle",
            20,
        );
        assert!(got.contains("needle"), "{got}");
    }

    #[test]
    fn ranks_title_hits_first_and_explains_other_matches() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::init(d.path(), "S", Utc::now()).unwrap();
        let mk = |title: &str, body: &str, labels: &[&str]| {
            ops::create_ticket(
                &s,
                NewTicket {
                    title: title.into(),
                    body: Some(body.into()),
                    list: Some("todo".into()),
                    labels: labels.iter().map(|l| l.to_string()).collect(),
                    ..Default::default()
                },
                "ada",
                Utc::now(),
            )
            .unwrap()
            .id
        };
        let a = mk("Foam rework", "", &["ocean"]);
        let b = mk("Shore waves", "the foam near the Shore looks static", &[]);
        let c = mk("Unrelated", "", &[]);
        ops::add_comment(&s, &c, "bob", "Blocked on FOAM shader compile", Utc::now()).unwrap();

        let hits = tickets(&s, &parse("foam"), "done").unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, [a.as_str(), b.as_str(), c.as_str()]);
        assert_eq!(hits[0].matched, None);
        assert_eq!(hits[1].matched.as_deref(), Some("body"));
        assert_eq!(
            hits[1].snippet.as_deref(),
            Some("the foam near the Shore looks static")
        );
        assert_eq!(hits[2].matched.as_deref(), Some("comment c-1 by bob"));

        assert_eq!(
            tickets(&s, &parse("foam label:ocean"), "done")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            tickets(&s, &parse("foam -static"), "done").unwrap().len(),
            2
        );
        assert_eq!(
            tickets(&s, &parse(&b.to_lowercase().replace('-', "")), "done").unwrap()[0].id,
            b
        );
        assert!(tickets(&s, &parse("foam is:done"), "done")
            .unwrap()
            .is_empty());
        assert!(tickets(&s, &parse("nothing-like-this"), "done")
            .unwrap()
            .is_empty());
    }
}
