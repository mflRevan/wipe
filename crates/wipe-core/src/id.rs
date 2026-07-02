//! Stable, human-friendly ID formatting.
//!
//! Ticket IDs look like `T-23`, comment IDs like `c-7`. IDs are allocated from
//! monotonic counters stored in the board / ticket, so they are deterministic and
//! never reused within a board.

/// Format a ticket ID from its numeric counter, e.g. `T-23`.
pub fn ticket_id(n: u64) -> String {
    format!("T-{n}")
}

/// Format a comment ID from its numeric counter, e.g. `c-7`.
pub fn comment_id(n: u64) -> String {
    format!("c-{n}")
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
    fn formats_ids() {
        assert_eq!(ticket_id(23), "T-23");
        assert_eq!(comment_id(7), "c-7");
    }

    #[test]
    fn slugs_names() {
        assert_eq!(slug("In Progress"), "in-progress");
        assert_eq!(slug("  To-Do!! "), "to-do");
        assert_eq!(slug("Backlog"), "backlog");
        assert_eq!(slug("***"), "list");
    }
}
