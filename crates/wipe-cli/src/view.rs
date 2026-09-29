//! Compact ticket rows: what `status`, `ticket list`, and write receipts print.
//!
//! A full ticket (body, every comment, every activity entry) is the right answer
//! to "show me T-7" and the wrong one to "what is open?" - on a real board it is
//! tens of kilobytes per list. These rows carry just enough to decide what to
//! open next, and their size does not grow with a ticket's history.

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

use wipe_core::model::{Board, ChecklistItem, Ticket};
use wipe_core::ops;

/// Every field a compact row can carry, in output order.
pub const FIELDS: &[&str] = &[
    "id",
    "title",
    "list",
    "labels",
    "assignees",
    "priority",
    "comments",
    "checklist",
    "criteria",
    "blocked_by",
    "updated",
    "last_by",
    "created",
    "body",
];

/// The fields a row carries when `--fields` is not given (`created` and `body`
/// are opt-in).
const DEFAULT_FIELDS: &[&str] = &[
    "id",
    "title",
    "list",
    "labels",
    "assignees",
    "priority",
    "comments",
    "checklist",
    "criteria",
    "blocked_by",
    "updated",
    "last_by",
];

/// Validate a `--fields` selection; empty means the defaults.
pub fn fields(requested: &[String]) -> Result<Vec<String>> {
    if requested.is_empty() {
        return Ok(DEFAULT_FIELDS.iter().map(|s| s.to_string()).collect());
    }
    let mut out = Vec::new();
    for f in requested {
        let f = f.trim().to_ascii_lowercase().replace('-', "_");
        if f.is_empty() {
            continue;
        }
        if !FIELDS.contains(&f.as_str()) {
            bail!("unknown field `{f}` - available: {}", FIELDS.join(", "));
        }
        if !out.contains(&f) {
            out.push(f);
        }
    }
    Ok(out)
}

fn progress(items: &[ChecklistItem]) -> Option<String> {
    (!items.is_empty()).then(|| {
        let done = items.iter().filter(|i| i.done).count();
        format!("{done}/{}", items.len())
    })
}

/// Who touched the ticket last (latest comment or activity entry).
fn last_by(t: &Ticket) -> Option<(&str, DateTime<Utc>)> {
    let a = t.activity.iter().map(|a| (a.actor.as_str(), a.ts));
    let c = t.comments.iter().map(|c| (c.author.as_str(), c.created));
    a.chain(c).max_by_key(|(_, ts)| *ts)
}

/// Build one compact row. Empty values are omitted so rows stay short; `list`
/// is the list id; `blocked_by` lists only blockers that are still open.
pub fn row(t: &Ticket, list: &str, board: &Board, done_list: &str, fields: &[String]) -> Value {
    let mut m = Map::new();
    for f in fields {
        let v = match f.as_str() {
            "id" => json!(t.id),
            "title" => json!(t.title),
            "list" => json!(list),
            "labels" if !t.labels.is_empty() => json!(t.labels),
            "assignees" if !t.assignees.is_empty() => json!(t.assignees),
            "priority" => match &t.priority {
                Some(p) => json!(p),
                None => continue,
            },
            "comments" if !t.comments.is_empty() => json!(t.comments.len()),
            "checklist" => match progress(&t.checklist) {
                Some(p) => json!(p),
                None => continue,
            },
            "criteria" => match progress(&t.acceptance) {
                Some(p) => json!(p),
                None => continue,
            },
            "blocked_by" => {
                let open = ops::open_blockers(t, board, done_list);
                if open.is_empty() {
                    continue;
                }
                json!(open)
            }
            "updated" => json!(t.updated.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            "last_by" => match last_by(t) {
                Some((who, _)) => json!(who),
                None => continue,
            },
            "created" => json!(t.created.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            "body" if !t.body.is_empty() => json!(t.body),
            _ => continue,
        };
        m.insert(f.clone(), v);
    }
    Value::Object(m)
}

/// Parse a `--since` value: RFC-3339, a bare date (midnight UTC), or a span ago
/// such as `30m`, `12h`, `2d`, `1w`.
pub fn parse_since(raw: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let s = raw.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Ok(d.with_timezone(&Utc));
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).expect("midnight").and_utc());
    }
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    if let Ok(n) = num.parse::<i64>() {
        let span = match unit {
            "m" | "min" => Some(chrono::Duration::minutes(n)),
            "h" => Some(chrono::Duration::hours(n)),
            "d" => Some(chrono::Duration::days(n)),
            "w" => Some(chrono::Duration::weeks(n)),
            _ => None,
        };
        if let Some(span) = span {
            return Ok(now - span);
        }
    }
    bail!(
        "cannot read `{raw}` as a time - use RFC-3339 (2026-09-29T08:00:00Z), a date \
         (2026-09-29), or a span ago (30m, 12h, 2d, 1w)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn since_accepts_timestamps_dates_and_spans() {
        let now = Utc.with_ymd_and_hms(2026, 9, 29, 12, 0, 0).unwrap();
        assert_eq!(
            parse_since("2026-09-01", now).unwrap(),
            Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap()
        );
        assert_eq!(
            parse_since("2026-09-28T10:00:00Z", now).unwrap(),
            Utc.with_ymd_and_hms(2026, 9, 28, 10, 0, 0).unwrap()
        );
        assert_eq!(
            parse_since("2d", now).unwrap(),
            now - chrono::Duration::days(2)
        );
        assert_eq!(
            parse_since("30m", now).unwrap(),
            now - chrono::Duration::minutes(30)
        );
        assert!(parse_since("yesterday", now).is_err());
        assert!(parse_since("5x", now).is_err());
    }

    #[test]
    fn unknown_fields_are_rejected_with_the_valid_set() {
        let err = fields(&["id".into(), "nope".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("nope") && err.contains("blocked_by"), "{err}");
        assert_eq!(
            fields(&["ID".into(), "last-by".into()]).unwrap(),
            vec!["id", "last_by"]
        );
        assert!(fields(&[]).unwrap().contains(&"title".to_string()));
    }
}
