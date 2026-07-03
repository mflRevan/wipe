//! Command-line surface for `wipe`, defined with `clap`'s derive API.
//!
//! Doc-comments on each command/field become the `--help` text, so the CLI is
//! self-documenting for both humans and agents.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Git-native task board for humans and agents.
///
/// `wipe` stores a Trello-style board as flat JSON under `.wipe/`, engineered for
/// clean git diffs. Agents drive it through this CLI (add `--json` to any command);
/// humans use the local UI via `wipe serve`.
#[derive(Debug, Parser)]
#[command(name = "wipe", version, about, long_about = None, propagate_version = true)]
pub struct Cli {
    /// Emit machine-readable JSON instead of human-formatted text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Run as if wipe was started in <PATH> instead of the current directory.
    #[arg(short = 'C', long = "cwd", global = true, value_name = "PATH")]
    pub cwd: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize a new wipe board in a directory.
    Init(InitArgs),
    /// Show the board at a glance.
    Status,
    /// Inspect and manage the board itself.
    #[command(subcommand)]
    Board(BoardCmd),
    /// Manage lists (the board's columns).
    #[command(subcommand)]
    List(ListCmd),
    /// Manage tickets (cards).
    #[command(subcommand)]
    Ticket(TicketCmd),
    /// Manage comments on tickets.
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Manage labels.
    #[command(subcommand)]
    Label(LabelCmd),
    /// Manage media/attachments referenced by tickets.
    #[command(subcommand)]
    Media(MediaCmd),
    /// Start the local web UI daemon.
    Serve(ServeArgs),
    /// Get or set settings (project by default; `--global` for user defaults).
    Config {
        /// Operate on the machine-wide user config instead of this board.
        #[arg(long)]
        global: bool,
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Diagnose the environment and the current board.
    Doctor,
    /// Print or install the agent SKILL guide for this CLI.
    Skill {
        #[command(subcommand)]
        cmd: Option<SkillCmd>,
    },
}

/// `wipe init`
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to initialize (defaults to the current directory).
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Board name (defaults to the directory name).
    #[arg(long)]
    pub name: Option<String>,
    /// Skip the interactive wizard, using defaults / your global config.
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// Starter content: `standard` (lists+labels), `lists`, or `empty`.
    #[arg(long, value_name = "KIND")]
    pub starter: Option<String>,
}

/// `wipe skill ...`
#[derive(Debug, Subcommand)]
pub enum SkillCmd {
    /// Print the SKILL.md guide to stdout (the default when no subcommand given).
    Show,
    /// Install SKILL.md into an agent skills directory.
    Install(SkillInstallArgs),
    /// Show where the skill would be installed, without writing anything.
    Path(SkillInstallArgs),
}

/// `wipe skill install` / `wipe skill path`
#[derive(Debug, Args, Clone)]
pub struct SkillInstallArgs {
    /// Skills convention: `claude` (.claude/skills), `agents` (.agents/skills),
    /// or omit to auto-detect from the project / home directory.
    #[arg(long, value_name = "TARGET")]
    pub target: Option<String>,
    /// Install user-globally (~/.claude or ~/.agents) instead of project-scoped.
    #[arg(long)]
    pub global: bool,
    /// Install under an explicit base directory (a `skills/` dir is created in it).
    #[arg(long, value_name = "PATH")]
    pub dir: Option<PathBuf>,
    /// Overwrite an existing SKILL.md if present.
    #[arg(long)]
    pub force: bool,
}

/// `wipe board ...`
#[derive(Debug, Subcommand)]
pub enum BoardCmd {
    /// Show board metadata.
    Show,
    /// Rename the board.
    Rename {
        /// New board name.
        name: String,
    },
}

/// `wipe list ...`
#[derive(Debug, Subcommand)]
pub enum ListCmd {
    /// Show all lists and their card counts.
    Show,
    /// Add a new list to the end of the board.
    Add {
        /// Display name of the list.
        name: String,
    },
    /// Rename a list (its ID stays stable).
    Rename {
        /// List ID (kebab-case slug).
        id: String,
        /// New display name.
        name: String,
    },
    /// Move a list to a new position (0-based).
    Move {
        /// List ID.
        id: String,
        /// Target index.
        index: usize,
    },
    /// Remove a list. Use --force to also delete its tickets.
    Remove {
        /// List ID.
        id: String,
        /// Delete contained tickets too.
        #[arg(long)]
        force: bool,
    },
}

/// `wipe ticket ...`
#[derive(Debug, Subcommand)]
pub enum TicketCmd {
    /// Create a ticket.
    Create(TicketCreateArgs),
    /// Show a ticket in full.
    Show {
        /// Ticket ID, e.g. T-1.
        id: String,
    },
    /// Edit a ticket's core fields.
    Edit(TicketEditArgs),
    /// Move a ticket to another list.
    Move {
        /// Ticket ID.
        id: String,
        /// Destination list ID.
        #[arg(long)]
        to: String,
        /// 0-based position within the list (appended if omitted).
        #[arg(long)]
        pos: Option<usize>,
    },
    /// Add or remove an assignee.
    Assign {
        /// Ticket ID.
        id: String,
        /// Assignee identity (e.g. "Ada <ada@example.com>" or an agent ID).
        who: String,
        /// Remove instead of add.
        #[arg(long)]
        remove: bool,
    },
    /// Move a ticket to the done list.
    Close {
        /// Ticket ID.
        id: String,
    },
    /// Move a ticket back to the first list.
    Reopen {
        /// Ticket ID.
        id: String,
    },
    /// Delete a ticket.
    Delete {
        /// Ticket ID.
        id: String,
        /// Do not require confirmation (always required in non-interactive use).
        #[arg(long)]
        yes: bool,
    },
    /// List tickets, optionally filtered.
    List(TicketListArgs),
}

/// `wipe ticket create`
#[derive(Debug, Args)]
pub struct TicketCreateArgs {
    /// Short title.
    #[arg(long, short)]
    pub title: String,
    /// Long-form body (Markdown allowed).
    #[arg(long, short)]
    pub body: Option<String>,
    /// Priority.
    #[arg(long)]
    pub priority: Option<String>,
    /// Destination list ID (defaults to the first list).
    #[arg(long, short = 'l')]
    pub list: Option<String>,
    /// Label to apply (repeatable).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Assignee (repeatable).
    #[arg(long = "assignee", value_name = "WHO")]
    pub assignees: Vec<String>,
}

/// `wipe ticket edit`
#[derive(Debug, Args)]
pub struct TicketEditArgs {
    /// Ticket ID.
    pub id: String,
    /// New title.
    #[arg(long)]
    pub title: Option<String>,
    /// New body.
    #[arg(long)]
    pub body: Option<String>,
    /// New priority.
    #[arg(long)]
    pub priority: Option<String>,
}

/// `wipe ticket list`
#[derive(Debug, Args)]
pub struct TicketListArgs {
    /// Only tickets on this list.
    #[arg(long)]
    pub list: Option<String>,
    /// Only tickets carrying this label.
    #[arg(long)]
    pub label: Option<String>,
}

/// `wipe comment ...`
#[derive(Debug, Subcommand)]
pub enum CommentCmd {
    /// Add a comment to a ticket.
    Add {
        /// Ticket ID.
        ticket: String,
        /// Comment body (Markdown allowed).
        #[arg(long, short)]
        body: String,
        /// Override the author identity (defaults to git config / $WIPE_AUTHOR).
        #[arg(long)]
        author: Option<String>,
    },
    /// List a ticket's comments.
    List {
        /// Ticket ID.
        ticket: String,
    },
}

/// `wipe label ...`
#[derive(Debug, Subcommand)]
pub enum LabelCmd {
    /// Define a new label.
    Create {
        /// Label name.
        name: String,
        /// Optional color (hex or token).
        #[arg(long)]
        color: Option<String>,
        /// Optional description.
        #[arg(long)]
        description: Option<String>,
    },
    /// List defined labels.
    List,
    /// Delete a label definition and strip it from all tickets.
    Delete {
        /// Label name.
        name: String,
    },
    /// Apply a label to a ticket.
    Assign {
        /// Ticket ID.
        ticket: String,
        /// Label name.
        name: String,
    },
    /// Remove a label from a ticket.
    Remove {
        /// Ticket ID.
        ticket: String,
        /// Label name.
        name: String,
    },
}

/// `wipe media ...`
#[derive(Debug, Subcommand)]
pub enum MediaCmd {
    /// Attach a file to a ticket (copied into .wipe/media/).
    Add {
        /// Ticket ID.
        ticket: String,
        /// Path to the file to attach.
        path: PathBuf,
    },
    /// List a ticket's attachments.
    List {
        /// Ticket ID.
        ticket: String,
    },
    /// Detach a file from a ticket.
    Remove {
        /// Ticket ID.
        ticket: String,
        /// Attachment file name.
        name: String,
    },
}

/// `wipe serve`
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Port to listen on (overrides settings.json).
    #[arg(long)]
    pub port: Option<u16>,
    /// Open the UI in a browser once started.
    #[arg(long)]
    pub open: bool,
    /// Auto-stop after N seconds with no viewers (0 = never; overrides settings).
    #[arg(long, value_name = "SECS")]
    pub idle: Option<u64>,
}

/// `wipe config ...`
#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Show all settings.
    Show,
    /// Get a setting by key (daemon.port, daemon.expose, board.name).
    Get {
        /// Setting key.
        key: String,
    },
    /// Set a setting by key.
    Set {
        /// Setting key.
        key: String,
        /// New value.
        value: String,
    },
}
