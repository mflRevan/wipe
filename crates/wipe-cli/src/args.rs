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

    /// Author authored actions as this identity for this command (typically an
    /// agent id). Overrides the session/VCS identity; see `wipe identity`.
    #[arg(long = "agentid", global = true, value_name = "ID")]
    pub agentid: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize a new wipe board in a directory.
    Init(InitArgs),
    /// Configure machine-wide defaults (a guided global setup).
    Onboard(OnboardArgs),
    /// Manage who your actions are attributed to (humans and agents).
    #[command(subcommand)]
    Identity(IdentityCmd),
    /// Discover `.wipe` boards on disk and add them to the local registry.
    Scan(ScanArgs),
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
    /// Manage a ticket's checklist (to-do items).
    #[command(subcommand)]
    Checklist(ChecklistCmd),
    /// Manage a ticket's acceptance criteria (the reviewer's checklist).
    #[command(subcommand, visible_alias = "acceptance")]
    Criteria(ChecklistCmd),
    /// Manage labels.
    #[command(subcommand)]
    Label(LabelCmd),
    /// Manage media/attachments referenced by tickets.
    #[command(subcommand)]
    Media(MediaCmd),
    /// Post to and search the project forum (git-tracked discussion threads).
    #[command(subcommand)]
    Forum(ForumCmd),
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
    /// Generate a shell completion script (bash, zsh, fish, powershell, elvish).
    Completions {
        /// Target shell.
        shell: clap_complete::Shell,
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

/// `wipe onboard`
#[derive(Debug, Args)]
pub struct OnboardArgs {
    /// Skip the interactive flow and just print the current global config.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// `wipe identity ...`
#[derive(Debug, Subcommand)]
pub enum IdentityCmd {
    /// List available identities (registry + VCS), marking the active one.
    ///
    /// Agents: run this FIRST to see whether an identity for you already exists
    /// before creating a new one with `wipe identity use`.
    List,
    /// Bind an identity to this terminal session (creates it if new).
    Use(IdentityUseArgs),
    /// Show who actions are currently attributed to, and why.
    Whoami,
    /// Unbind this session's identity (revert to VCS/default resolution).
    Clear,
}

/// `wipe identity use`
#[derive(Debug, Args)]
pub struct IdentityUseArgs {
    /// Identity id to use (an existing id, an email, or a fresh agent slug).
    pub id: String,
    /// Display name (defaults to the id).
    #[arg(long)]
    pub name: Option<String>,
    /// Mark this identity as an agent (default when the id isn't an email).
    #[arg(long)]
    pub agent: bool,
    /// Mark this identity as a human.
    #[arg(long, conflicts_with = "agent")]
    pub human: bool,
}

/// `wipe scan`
#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Root directory to scan (repeatable; defaults to your configured scan roots
    /// or your home directory).
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    /// How many directory levels deep to search.
    #[arg(long, default_value = "7")]
    pub depth: usize,
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

/// `wipe checklist ...` and `wipe criteria ...` - the two tickable surfaces on a
/// ticket share the same verbs (checklist items are `ck-<n>`, criteria `ac-<n>`).
#[derive(Debug, Subcommand)]
pub enum ChecklistCmd {
    /// Add an item.
    Add {
        /// Ticket ID.
        ticket: String,
        /// Item text.
        #[arg(long, short)]
        text: String,
    },
    /// List a ticket's items and their state.
    List {
        /// Ticket ID.
        ticket: String,
    },
    /// Check an item off (mark done).
    Check {
        /// Ticket ID.
        ticket: String,
        /// Item ID (e.g. ck-1 or ac-1).
        item: String,
    },
    /// Uncheck an item (mark not done).
    Uncheck {
        /// Ticket ID.
        ticket: String,
        /// Item ID (e.g. ck-1 or ac-1).
        item: String,
    },
    /// Toggle an item's checked state.
    Toggle {
        /// Ticket ID.
        ticket: String,
        /// Item ID (e.g. ck-1 or ac-1).
        item: String,
    },
    /// Edit an item's text.
    Edit {
        /// Ticket ID.
        ticket: String,
        /// Item ID (e.g. ck-1 or ac-1).
        item: String,
        /// New text.
        #[arg(long, short)]
        text: String,
    },
    /// Remove an item.
    Remove {
        /// Ticket ID.
        ticket: String,
        /// Item ID (e.g. ck-1 or ac-1).
        item: String,
    },
    /// Move an item to a new 0-based position.
    Move {
        /// Ticket ID.
        ticket: String,
        /// Item ID (e.g. ck-1 or ac-1).
        item: String,
        /// Target index (0-based).
        index: usize,
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

/// `wipe forum ...`
#[derive(Debug, Subcommand)]
pub enum ForumCmd {
    /// Open a new thread with a root post.
    Post(ForumPostArgs),
    /// Reply to a post at any depth (parent is a post ID like F-1 or F-1.2).
    Reply(ForumReplyArgs),
    /// Show a thread (or a subtree) as an indented tree.
    Show {
        /// Thread or post ID (e.g. F-1 or F-1.2).
        id: String,
        /// Limit how deep to render (relative to the shown post).
        #[arg(long)]
        depth: Option<usize>,
    },
    /// List threads, newest first.
    List {
        /// Only threads whose root carries this label.
        #[arg(long)]
        label: Option<String>,
        /// Only threads whose root was posted by this author (substring).
        #[arg(long)]
        author: Option<String>,
        /// Cap the number of threads shown.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Search posts by regex pattern and/or filters.
    Search(ForumSearchArgs),
    /// Edit a post's body.
    Edit {
        /// Post ID.
        id: String,
        /// New body (Markdown allowed).
        #[arg(long, short)]
        body: String,
    },
    /// Delete a post and its entire subtree (root deletes the whole thread).
    Delete {
        /// Post or thread ID.
        id: String,
        /// Required to actually delete (subtree deletion is irreversible).
        #[arg(long)]
        yes: bool,
    },
    /// Watch the forum and stream new posts as newline-delimited JSON events.
    ///
    /// Blocks and prints one JSON object per new matching post; agent harnesses
    /// run this and react to each line. Stop it with Ctrl-C.
    Watch(ForumWatchArgs),
}

/// `wipe forum post`
#[derive(Debug, Args)]
pub struct ForumPostArgs {
    /// Thread title (headline).
    #[arg(long, short)]
    pub title: String,
    /// Message body (Markdown allowed).
    #[arg(long, short)]
    pub body: Option<String>,
    /// Label to apply (repeatable), from the board's label pool.
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Reference to include (ticket ID, post ID, or URL; repeatable).
    #[arg(long = "ref", value_name = "REF")]
    pub refs: Vec<String>,
    /// File to attach (repeatable).
    #[arg(long = "attach", value_name = "PATH")]
    pub attach: Vec<PathBuf>,
    /// Override the author identity (defaults to git config / $WIPE_AUTHOR).
    #[arg(long)]
    pub author: Option<String>,
}

/// `wipe forum reply`
#[derive(Debug, Args)]
pub struct ForumReplyArgs {
    /// Parent post ID (e.g. F-1 or F-1.2).
    pub id: String,
    /// Reply body (Markdown allowed).
    #[arg(long, short)]
    pub body: String,
    /// Label to apply (repeatable).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Reference to include (repeatable).
    #[arg(long = "ref", value_name = "REF")]
    pub refs: Vec<String>,
    /// File to attach (repeatable).
    #[arg(long = "attach", value_name = "PATH")]
    pub attach: Vec<PathBuf>,
    /// Override the author identity.
    #[arg(long)]
    pub author: Option<String>,
}

/// `wipe forum search`
#[derive(Debug, Args)]
pub struct ForumSearchArgs {
    /// Regex to match against post bodies (or titles with --titles). Optional.
    pub pattern: Option<String>,
    /// Only posts by this author (substring, case-insensitive).
    #[arg(long)]
    pub author: Option<String>,
    /// Require this label (repeatable; all must be present).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Restrict to a thread or subtree by ID (e.g. F-1 or F-1.2).
    #[arg(long)]
    pub scope: Option<String>,
    /// Only match posts at this depth or shallower (root = 0).
    #[arg(long)]
    pub depth: Option<usize>,
    /// Match only thread titles (root posts).
    #[arg(long)]
    pub titles: bool,
    /// Cap the number of results.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Make the pattern case-sensitive (default: case-insensitive).
    #[arg(long = "case-sensitive")]
    pub case_sensitive: bool,
}

/// `wipe forum watch`
#[derive(Debug, Args)]
pub struct ForumWatchArgs {
    /// Only emit posts whose body matches this regex.
    #[arg(long)]
    pub pattern: Option<String>,
    /// Only emit posts by this author (substring).
    #[arg(long)]
    pub author: Option<String>,
    /// Only emit posts carrying this label (repeatable).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Only emit posts within this thread/subtree.
    #[arg(long)]
    pub scope: Option<String>,
    /// Poll interval in milliseconds.
    #[arg(long, default_value = "1000")]
    pub interval: u64,
    /// Emit all currently-matching posts once before watching for new ones.
    #[arg(long)]
    pub replay: bool,
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
