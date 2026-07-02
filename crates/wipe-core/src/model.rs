//! The wipe domain model.
//!
//! These types map 1:1 onto the JSON files under `.wipe/`. Field order is
//! significant: `serde_json` serializes struct fields in declaration order, and we
//! rely on that (plus `Vec` ordering and no hash maps) to keep on-disk output
//! deterministic. Optional/empty fields are skipped so diffs stay minimal.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::id::slug;

/// On-disk format version. Bumped when the JSON schema changes in a
/// backwards-incompatible way; every top-level file carries it for migration.
pub const FORMAT_VERSION: u32 = 1;

/// Default port the local daemon listens on when the user hasn't chosen one.
pub const DEFAULT_PORT: u16 = 6737;

// ---------------------------------------------------------------------------
// board.json
// ---------------------------------------------------------------------------

/// The board — the top-level object of a project. Holds ordered [`List`]s whose
/// `cards` reference ticket IDs. Ticket *content* lives in separate files under
/// `tickets/`, so moving a card and editing a ticket never touch the same file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    /// On-disk format version.
    pub version: u32,
    /// Stable unique board ID (UUID v4).
    pub id: String,
    /// Human-readable board name.
    pub name: String,
    /// Optional longer description (Markdown allowed).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Ordered lists (columns) of the board.
    pub lists: Vec<List>,
    /// Next ticket counter; `T-<next_ticket>` is the next ID to allocate.
    pub next_ticket: u64,
    /// When the board was created.
    pub created: DateTime<Utc>,
    /// When the board was last modified.
    pub updated: DateTime<Utc>,
}

impl Board {
    /// Create a fresh board with the default set of lists.
    pub fn new(name: impl Into<String>, now: DateTime<Utc>) -> Self {
        Board {
            version: FORMAT_VERSION,
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: String::new(),
            lists: default_lists(),
            next_ticket: 1,
            created: now,
            updated: now,
        }
    }

    /// Find a list by ID.
    pub fn list(&self, id: &str) -> Option<&List> {
        self.lists.iter().find(|l| l.id == id)
    }

    /// Find a list by ID (mutable).
    pub fn list_mut(&mut self, id: &str) -> Option<&mut List> {
        self.lists.iter_mut().find(|l| l.id == id)
    }

    /// Return `(list_id, index)` of the list currently containing `ticket_id`.
    pub fn locate_card(&self, ticket_id: &str) -> Option<(String, usize)> {
        for list in &self.lists {
            if let Some(idx) = list.cards.iter().position(|c| c == ticket_id) {
                return Some((list.id.clone(), idx));
            }
        }
        None
    }
}

/// A list (column) on the board. `cards` is the ordered set of ticket IDs it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    /// Stable list ID (kebab-case slug of the original name).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Optional UI color (hex or token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Optional work-in-progress limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wip_limit: Option<u32>,
    /// Ordered ticket IDs contained in this list.
    #[serde(default)]
    pub cards: Vec<String>,
}

impl List {
    /// Create an empty list from a display name.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        List {
            id: slug(&name),
            name,
            color: None,
            wip_limit: None,
            cards: Vec::new(),
        }
    }
}

/// The default lists created by `wipe init`.
fn default_lists() -> Vec<List> {
    ["Backlog", "Todo", "In Progress", "Done"]
        .into_iter()
        .map(List::new)
        .collect()
}

// ---------------------------------------------------------------------------
// tickets/T-###.json
// ---------------------------------------------------------------------------

/// A ticket (card). Stored as its own file; comments are inline and short.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticket {
    /// On-disk format version.
    pub version: u32,
    /// Ticket ID, e.g. `T-23`.
    pub id: String,
    /// Short title.
    pub title: String,
    /// Long-form body (Markdown allowed inside the JSON string).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    /// Ticket type (references a name in `definitions.json`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Priority (references a name in `definitions.json`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// Applied label names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Applied free-form tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Assignee identities (git-style `Name <email>` or agent IDs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
    /// Relations to other tickets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
    /// Referenced media/attachment file names under `.wipe/media/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,
    /// Inline comment thread.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
    /// Next comment counter for this ticket.
    #[serde(default = "one")]
    pub next_comment: u64,
    /// When the ticket was created.
    pub created: DateTime<Utc>,
    /// When the ticket was last modified.
    pub updated: DateTime<Utc>,
}

fn one() -> u64 {
    1
}

impl Ticket {
    /// Create a new ticket with the given ID and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>, now: DateTime<Utc>) -> Self {
        Ticket {
            version: FORMAT_VERSION,
            id: id.into(),
            title: title.into(),
            body: String::new(),
            kind: None,
            priority: None,
            labels: Vec::new(),
            tags: Vec::new(),
            assignees: Vec::new(),
            relations: Vec::new(),
            attachments: Vec::new(),
            comments: Vec::new(),
            next_comment: 1,
            created: now,
            updated: now,
        }
    }

    /// Append a comment, allocating the next comment ID. Returns the new comment ID.
    pub fn add_comment(
        &mut self,
        author: impl Into<String>,
        body: impl Into<String>,
        now: DateTime<Utc>,
    ) -> String {
        let id = crate::id::comment_id(self.next_comment);
        self.next_comment += 1;
        self.comments.push(Comment {
            id: id.clone(),
            author: author.into(),
            body: body.into(),
            created: now,
            edited: None,
        });
        self.updated = now;
        id
    }
}

/// A relation from one ticket to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    /// Kind of relation.
    pub kind: RelationKind,
    /// Target ticket ID.
    pub target: String,
}

/// The kind of a [`Relation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationKind {
    /// This ticket blocks the target.
    Blocks,
    /// This ticket is blocked by the target.
    BlockedBy,
    /// The target is a parent of this ticket.
    Parent,
    /// The target is a child of this ticket.
    Child,
    /// A soft relationship.
    Relates,
}

/// An inline comment on a ticket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    /// Comment ID, e.g. `c-7`.
    pub id: String,
    /// Author identity (git `Name <email>` or agent ID).
    pub author: String,
    /// Comment body (Markdown allowed).
    pub body: String,
    /// When the comment was posted.
    pub created: DateTime<Utc>,
    /// When the comment was last edited, if ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// definitions.json
// ---------------------------------------------------------------------------

/// Project-wide registries: ticket types, labels, tags, priorities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Definitions {
    /// On-disk format version.
    pub version: u32,
    /// Allowed ticket types.
    #[serde(default)]
    pub types: Vec<String>,
    /// Defined labels.
    #[serde(default)]
    pub labels: Vec<LabelDef>,
    /// Registered free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Allowed priorities, ordered from lowest to highest.
    #[serde(default)]
    pub priorities: Vec<String>,
}

impl Definitions {
    /// A sensible default set of definitions for a new board.
    pub fn seed() -> Self {
        Definitions {
            version: FORMAT_VERSION,
            types: vec![
                "feature".into(),
                "bug".into(),
                "chore".into(),
                "spec".into(),
            ],
            labels: vec![
                LabelDef::new("blocked", Some("#e5484d")),
                LabelDef::new("needs-review", Some("#f5a623")),
                LabelDef::new("agent", Some("#8e4ec6")),
            ],
            tags: Vec::new(),
            priorities: vec![
                "low".into(),
                "medium".into(),
                "high".into(),
                "urgent".into(),
            ],
        }
    }
}

/// A label definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelDef {
    /// Label name (unique within the board).
    pub name: String,
    /// Optional UI color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Optional description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl LabelDef {
    /// Create a label with a name and optional color.
    pub fn new(name: impl Into<String>, color: Option<&str>) -> Self {
        LabelDef {
            name: name.into(),
            color: color.map(|c| c.to_string()),
            description: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// settings.json
// ---------------------------------------------------------------------------

/// Project settings, including how the local daemon is exposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// On-disk format version.
    pub version: u32,
    /// Local daemon settings.
    #[serde(default)]
    pub daemon: DaemonSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: FORMAT_VERSION,
            daemon: DaemonSettings::default(),
        }
    }
}

/// Configuration for the local daemon that serves the human UX.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonSettings {
    /// Port to listen on.
    pub port: u16,
    /// How the daemon is exposed beyond localhost.
    #[serde(default)]
    pub expose: Exposure,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        DaemonSettings {
            port: DEFAULT_PORT,
            expose: Exposure::default(),
        }
    }
}

/// How the local daemon is reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Exposure {
    /// Localhost only.
    #[default]
    None,
    /// Advertised over a Tailscale network.
    Tailscale,
    /// Behind a user-provided reverse proxy.
    Proxy,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0).unwrap()
    }

    #[test]
    fn board_has_default_lists() {
        let b = Board::new("Demo", fixed());
        assert_eq!(b.lists.len(), 4);
        assert_eq!(b.lists[2].id, "in-progress");
        assert_eq!(b.next_ticket, 1);
    }

    #[test]
    fn ticket_type_serializes_as_type() {
        let mut t = Ticket::new("T-1", "Hello", fixed());
        t.kind = Some("bug".into());
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"type\":\"bug\""));
        // Empty vecs and empty body are skipped.
        assert!(!json.contains("labels"));
        assert!(!json.contains("\"body\""));
    }

    #[test]
    fn comment_allocation_is_monotonic() {
        let mut t = Ticket::new("T-1", "Hello", fixed());
        let a = t.add_comment("me", "first", fixed());
        let b = t.add_comment("me", "second", fixed());
        assert_eq!(a, "c-1");
        assert_eq!(b, "c-2");
        assert_eq!(t.next_comment, 3);
    }

    #[test]
    fn relation_kind_is_kebab_case() {
        let r = Relation {
            kind: RelationKind::BlockedBy,
            target: "T-2".into(),
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"kind":"blocked-by","target":"T-2"}"#
        );
    }
}
