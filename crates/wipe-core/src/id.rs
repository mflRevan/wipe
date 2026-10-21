//! Stable, human-friendly ID formatting.
//!
//! Ticket IDs look like `T-017` (three or more upper-case hex digits, zero
//! padded) on current boards and `T-23` (decimal) on boards created before 0.4.1;
//! comment IDs look like `c-7`. IDs are allocated from monotonic counters stored
//! in the board / ticket, so they are deterministic and never reused within a
//! board.

/// Format a legacy (decimal) ticket ID from its counter, e.g. `T-23`.
pub fn ticket_id(n: u64) -> String {
    format!("T-{n}")
}

/// Format a hex ticket ID from its counter: at least three upper-case hex
/// digits, zero padded - `T-001`, `T-0FF`, `T-2AF`, `T-1000`.
pub fn hex_ticket_id(n: u64) -> String {
    format!("T-{n:03X}")
}

/// The digits of a ticket reference as a person or agent types it: `T-2AF`,
/// `t2af`, `T 005`, `#T-12` -> `"2AF"`, `"2AF"`, `"005"`, `"12"` (upper-cased).
/// `None` when it isn't shaped like a ticket id.
pub fn ticket_ref_digits(raw: &str) -> Option<String> {
    let s = raw.trim().trim_start_matches('#');
    let rest = s.strip_prefix('T').or_else(|| s.strip_prefix('t'))?;
    let digits = rest.trim_start_matches(['-', ' ', '_']);
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| digits.to_ascii_uppercase())
}

/// Format a comment ID from its numeric counter, e.g. `c-7`.
pub fn comment_id(n: u64) -> String {
    format!("c-{n}")
}

/// Format a checklist-item ID from its numeric counter, e.g. `ck-3`.
pub fn checklist_id(n: u64) -> String {
    format!("ck-{n}")
}

/// Format an acceptance-criterion ID from its numeric counter, e.g. `ac-2`.
pub fn acceptance_id(n: u64) -> String {
    format!("ac-{n}")
}

/// Generate a fresh, URL-safe bearer token (a hyphen-free UUIDv4), used to guard
/// an exposed daemon (`wipe serve --expose ...`).
pub fn token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Turn a human name into a stable kebab-case slug used for list IDs.
///
/// Non-alphanumeric runs collapse to a single `-`; the result is lowercased and
/// trimmed of leading/trailing dashes. Empty input yields `"list"`.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "list".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_ids_parses_refs_and_slugs_names() {
        assert_eq!(ticket_id(23), "T-23");
        assert_eq!(hex_ticket_id(1), "T-001");
        assert_eq!(hex_ticket_id(0x2AF), "T-2AF");
        assert_eq!(hex_ticket_id(0x1000), "T-1000");
        for (raw, want) in [
            ("t2af", Some("2AF")),
            ("T-005", Some("005")),
            (" #T 12 ", Some("12")),
            ("Tx1", None),
            ("T-", None),
        ] {
            assert_eq!(ticket_ref_digits(raw).as_deref(), want, "{raw}");
        }
        assert_eq!(
            (comment_id(7), checklist_id(3), acceptance_id(2)),
            ("c-7".into(), "ck-3".into(), "ac-2".into())
        );
        for (name, s) in [
            ("In Progress", "in-progress"),
            ("  To-Do!! ", "to-do"),
            ("***", "list"),
        ] {
            assert_eq!(slug(name), s);
        }
    }
}
