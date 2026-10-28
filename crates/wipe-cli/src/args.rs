//! Command-line surface for `wipe`, defined with `clap`'s derive API.
//!
//! Doc-comments on each command/field become the `--help` text, so the CLI is
//! self-documenting for both humans and agents.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Git-native task board for humans and agents.
///
/// The board lives as JSON under `.wipe/` and travels with git. Start with
/// `wipe status`; every view ends with the commands that go deeper.
#[derive(Debug, Parser)]
#[command(
    name = "wipe",
    version,
    about,
    long_about = None,
    after_help = "Global flags (any command): --json  machine-readable output  ·  -C <dir>  run in <dir>  ·\n\
                  --agentid <id>  act as this identity  ·  --echo  writes print the full object  ·  --pretty  indent JSON"
)]
pub struct Cli {
    /// Machine-readable JSON output (compact, one line).
    #[arg(long, global = true)]
    pub json: bool,

    /// Run as if wipe was started in <PATH> instead of the current directory.
    #[arg(
        short = 'C',
        long = "cwd",
        global = true,
        value_name = "PATH",
        hide = true
    )]
    pub cwd: Option<PathBuf>,

    /// Author authored actions as this identity for this command (typically an
    /// agent id). Overrides the session/VCS identity; see `wipe identity`.
    #[arg(long = "agentid", global = true, value_name = "ID", hide = true)]
    pub agentid: Option<String>,

    /// On writes, print the full updated object instead of the short receipt.
    #[arg(long, global = true, hide = true)]
    pub echo: bool,

    /// Pretty-print `--json` output (indented, multi-line).
    #[arg(long, global = true, hide = true)]
    pub pretty: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// The board at a glance: every list and its tickets, one line each.
    Status(StatusArgs),
    /// Search tickets and forum posts: words, "phrases", -exclude, ticket ids,
    /// label:, list:, @who, author:, priority:, is:open|done|blocked|ready.
    Search(SearchArgs),
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
    /// Manage lists (the board's columns).
    #[command(subcommand)]
    List(ListCmd),
    /// Wait for changes and print each as one line (`--until`: exit on the first).
    /// Your own changes are skipped.
    Watch(WatchArgs),
    /// What others changed on things you're assigned to, authored or subscribed
    /// to (`--all`: the whole board) - returns at once, newest first.
    Inbox(InboxArgs),
    /// Post to and search the project forum (git-tracked discussion threads).
    #[command(subcommand)]
    Forum(ForumCmd),
    /// Subscribe your identity to a ticket, list, or forum thread (for the inbox).
    Subscribe(SubscribeArgs),
    /// Unsubscribe your identity from a ticket, list, or forum thread.
    Unsubscribe(SubscribeArgs),
    /// List your identity's subscriptions.
    Subscriptions {
        /// Show another identity's subscriptions instead of yours.
        #[arg(long)]
        author: Option<String>,
    },
    /// Manage media/attachments referenced by tickets.
    #[command(subcommand)]
    Media(MediaCmd),
    /// Manage the trash: list, restore, or permanently purge deleted tickets.
    Trash {
        #[command(subcommand)]
        cmd: TrashCmd,
    },
    /// Commit the board's `.wipe/` changes as one atomic, wipe-attributed commit.
    Commit(CommitArgs),
    /// Manage who your actions are attributed to (humans and agents).
    #[command(subcommand)]
    Identity(IdentityCmd),
    /// Initialize a new wipe board in a directory.
    Init(InitArgs),
    /// Inspect and manage the board itself.
    #[command(subcommand)]
    Board(BoardCmd),
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
    /// Update wipe through the channel it was installed with (npm, installer, cargo).
    Update(UpdateArgs),
    /// Start the local web UI daemon.
    Serve(ServeArgs),
    /// Run the board server as a tray / menu-bar app (Windows, macOS).
    Tray(TrayArgs),
    /// Discover `.wipe` boards on disk and add them to the local registry.
    Scan(ScanArgs),
    /// Configure machine-wide defaults (a guided global setup).
    Onboard(OnboardArgs),
    /// Print or install a short agent guide (the CLI's hints teach the rest).
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

/// `wipe update`
#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Only show how wipe was installed, the update command, and what runs from it.
    #[arg(long)]
    pub check: bool,
    /// Close other wipe processes running from this installation without asking.
    #[arg(short, long)]
    pub yes: bool,
    /// Reinstall even when already on the latest version.
    #[arg(long)]
    pub force: bool,
}

/// `wipe status`
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Include every ticket's full content (body, comments, activity) - the
    /// pre-0.4 output. Large on real boards; prefer `wipe ticket show <id>`.
    #[arg(long)]
    pub full: bool,
    /// Also list the tickets on the done list instead of just counting them.
    #[arg(long)]
    pub all: bool,
    /// Skip a list entirely (repeatable).
    #[arg(long = "exclude-list", value_name = "LIST")]
    pub exclude_lists: Vec<String>,
    /// JSON: comma-separated fields per row (see `wipe ticket list --help`).
    #[arg(long, value_delimiter = ',', value_name = "FIELDS")]
    pub fields: Vec<String>,
}

/// `wipe subscribe` / `wipe unsubscribe`
#[derive(Debug, Args)]
pub struct SubscribeArgs {
    /// What to watch: a ticket (`T-3`), a list (`todo`), a forum thread (`F-2`),
    /// `forum` (all forum), or `all` (everything on the board).
    pub target: String,
    /// Act as this identity instead of the resolved one.
    #[arg(long)]
    pub author: Option<String>,
}

/// `wipe search`
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// The query, e.g. `foam shader`, `"storm waves" -wip`, `label:bug is:open`,
    /// `@me list:review`, `T-02A`.
    #[arg(required = true, num_args = 1.., value_name = "QUERY")]
    pub query: Vec<String>,
    /// Cap the results [default: 10; 0 = no cap].
    #[arg(long)]
    pub limit: Option<usize>,
    /// Only tickets.
    #[arg(long, conflicts_with = "forum")]
    pub tickets: bool,
    /// Only forum posts.
    #[arg(long)]
    pub forum: bool,
}

/// `wipe watch`
#[derive(Debug, Args)]
#[command(after_help = "Examples:
  wipe watch T-02A --until                 block until someone else touches T-02A
  wipe watch --from review --until         ... until a ticket leaves review (approved/rejected)
  wipe watch review --on commented,moved   stream comments and moves on the review list
  wipe watch forum                         stream new forum posts
  wipe watch T-02A --until --timeout 30m   give up after 30 minutes (exit 124)
Agent harnesses: run it in the background and treat each stdout line (or the
exit of an --until watch) as the notification.")]
pub struct WatchArgs {
    /// What to watch: tickets (`T-02A`), lists (`review`), forum threads
    /// (`F-3`), `forum`. Several are OR-ed; none = the whole board.
    #[arg(value_name = "TARGET")]
    pub targets: Vec<String>,
    /// Only these events (comma-separated): created, moved, commented, edited,
    /// labeled, assigned, blocked, attached, checked, deleted, posted.
    #[arg(long, value_delimiter = ',', value_name = "EVENTS")]
    pub on: Vec<String>,
    /// Only moves out of this list (e.g. `--from review`).
    #[arg(long, value_name = "LIST")]
    pub from: Option<String>,
    /// Only moves into this list.
    #[arg(long, value_name = "LIST")]
    pub to: Option<String>,
    /// Only changes by this identity (substring).
    #[arg(long, value_name = "WHO")]
    pub by: Option<String>,
    /// Exit after the first matching change (exit 0).
    #[arg(long, visible_alias = "once")]
    pub until: bool,
    /// Exit after this many matching changes.
    #[arg(long, value_name = "N", conflicts_with = "until")]
    pub count: Option<usize>,
    /// Give up after this long (`90s`, `10m`, `2h`); exit 124 if nothing matched.
    #[arg(long, value_name = "DURATION")]
    pub timeout: Option<String>,
    /// Also report your own changes.
    #[arg(long)]
    pub include_self: bool,
    /// Poll interval in milliseconds.
    #[arg(long, default_value = "1000", hide = true)]
    pub interval: u64,
}

/// `wipe inbox`
#[derive(Debug, Args)]
pub struct InboxArgs {
    /// Only events after this time: RFC-3339, a date (2026-09-01), or a span
    /// ago (`30m`, `12h`, `2d`, `1w`).
    #[arg(long)]
    pub since: Option<String>,
    /// Use *and advance* your stored read-cursor: show only what's new since you
    /// last read, then mark everything up to now as read.
    #[arg(long)]
    pub unread: bool,
    /// Act as this identity instead of the resolved one.
    #[arg(long)]
    pub author: Option<String>,
    /// Cap the events shown, newest first (`0` = no cap). The JSON `total`
    /// says how many there were.
    #[arg(long, default_value = "20")]
    pub limit: usize,
    /// Everything anyone else changed on the board, not just what you are
    /// assigned to, authored, or subscribed to ("what happened while I was away").
    #[arg(long)]
    pub all: bool,
}

/// `wipe commit`
#[derive(Debug, Args)]
pub struct CommitArgs {
    /// What to commit: a ticket id (e.g. `T-3`) to commit just that ticket's
    /// file, or omit to commit all of `.wipe/`.
    pub target: Option<String>,
    /// Commit message (a sensible default is used if omitted).
    #[arg(long, short)]
    pub message: Option<String>,
    /// Attribute the commit to this identity instead of the resolved one.
    #[arg(long)]
    pub author: Option<String>,
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
    /// Bind an identity to this terminal tab / agent session (creates it if new).
    /// Each new terminal or agent session starts with no identity.
    Use(IdentityUseArgs),
    /// Show who actions are currently attributed to, and why.
    Whoami,
    /// Unbind this session's identity (writes are refused until one is chosen).
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
    /// Convert a board's legacy decimal ticket ids (`T-23`) to the fixed-width
    /// hex format (`T-017`). Renames ticket files and rewrites every reference;
    /// old ids keep resolving as aliases. Commit/merge open branches first.
    TranslateIds {
        /// Don't ask for confirmation (required when not on a terminal).
        #[arg(long, short = 'y')]
        yes: bool,
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
    /// Show a ticket in full (plus commits that mention it, in git repos).
    Show(TicketShowArgs),
    /// Edit a ticket: fields, labels, assignees, blockers, list, and a comment -
    /// all in one call.
    Edit(TicketEditArgs),
    /// Move a ticket to another list: `wipe ticket move T-02A review`.
    Move {
        /// Ticket ID.
        id: String,
        /// Destination list ID (or pass it with --to).
        #[arg(value_name = "LIST", required_unless_present = "to")]
        list: Option<String>,
        /// Destination list ID.
        #[arg(long, conflicts_with = "list")]
        to: Option<String>,
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
    /// Hand work in for review: comment + move to the review list. The comment
    /// can carry what was tested and what was NOT.
    Submit(TicketSubmitArgs),
    /// Accept reviewed work: move it to the done list (refuses while acceptance
    /// criteria are unmet, unless --force), with an optional comment.
    Approve(TicketApproveArgs),
    /// Send reviewed work back: a reason is required; comment + move to the
    /// rework list (todo by default).
    Reject(TicketRejectArgs),
    /// Mark a ticket as blocked by other tickets (`--by T-3`, repeatable).
    Block(TicketBlockArgs),
    /// Remove blocked-by links (`--by T-3`, repeatable).
    Unblock(TicketBlockArgs),
    /// Move a ticket back to the first list.
    Reopen {
        /// Ticket ID.
        id: String,
    },
    /// Delete a ticket (moved to the restorable trash unless `--purge`).
    Delete {
        /// Ticket ID.
        id: String,
        /// Do not require confirmation (always required in non-interactive use).
        #[arg(long)]
        yes: bool,
        /// Permanently delete instead of moving it to the trash.
        #[arg(long)]
        purge: bool,
    },
    /// Duplicate a ticket (a copy on the same list, right after the original).
    Duplicate {
        /// Ticket ID.
        id: String,
    },
    /// List tickets, optionally filtered.
    List(TicketListArgs),
}

/// `wipe trash ...` - the restorable, gitignored bin for deleted tickets.
#[derive(Debug, Subcommand)]
pub enum TrashCmd {
    /// List trashed tickets, newest deletion first (purging expired ones first).
    List,
    /// Restore a trashed ticket back onto the board.
    Restore {
        /// Ticket ID.
        id: String,
    },
    /// Permanently delete one trashed ticket (or all with no id).
    Purge {
        /// Ticket ID (omit to empty the whole trash).
        id: Option<String>,
    },
}

/// `wipe ticket create`
#[derive(Debug, Args)]
pub struct TicketCreateArgs {
    /// Short title (or pass it with --title).
    #[arg(value_name = "TITLE")]
    pub title_pos: Option<String>,
    /// Short title.
    #[arg(long, short, conflicts_with = "title_pos")]
    pub title: Option<String>,
    /// Long-form body (Markdown allowed). `-` reads it from stdin.
    #[arg(long, short, allow_hyphen_values = true)]
    pub body: Option<String>,
    /// Read the body from a file (`-` = stdin). Safe for multi-line text on every
    /// platform and shell.
    #[arg(long, value_name = "PATH", conflicts_with = "body")]
    pub body_file: Option<PathBuf>,
    /// Priority.
    #[arg(long)]
    pub priority: Option<String>,
    /// Destination list ID (REQUIRED - e.g. `todo`; see `wipe list show`).
    #[arg(long, short = 'l')]
    pub list: Option<String>,
    /// Label to apply (repeatable).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Assignee (repeatable).
    #[arg(long = "assignee", value_name = "WHO")]
    pub assignees: Vec<String>,
    /// Ticket this one waits on (repeatable).
    #[arg(long = "blocked-by", value_name = "ID")]
    pub blocked_by: Vec<String>,
}

/// `wipe ticket show`
#[derive(Debug, Args)]
pub struct TicketShowArgs {
    /// Ticket ID (`T-02A`, `t2a`, or a pre-translation `T-42`).
    pub id: String,
    /// How many of the newest comments to show: a number or `all` [default: 3].
    #[arg(long, value_name = "N|all")]
    pub comments: Option<String>,
    /// Everything, unabridged: full body and comments, original wording,
    /// activity log, all commits.
    #[arg(long)]
    pub full: bool,
    /// Include the activity log (moves, edits, labels, checks...).
    #[arg(long)]
    pub activity: bool,
    /// Only the comments.
    #[arg(long)]
    pub comments_only: bool,
    #[arg(long, hide = true)]
    pub no_activity: bool,
    /// Leave out the commits that mention this ticket.
    #[arg(long)]
    pub no_commits: bool,
}

/// `wipe ticket edit`
#[derive(Debug, Args)]
pub struct TicketEditArgs {
    /// Ticket ID.
    pub id: String,
    /// New title.
    #[arg(long, short)]
    pub title: Option<String>,
    /// New body. `-` reads it from stdin. The creator's original wording is kept
    /// the first time someone else rewrites it (see `ticket show`).
    #[arg(long, short, allow_hyphen_values = true)]
    pub body: Option<String>,
    /// Read the new body from a file (`-` = stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "body")]
    pub body_file: Option<PathBuf>,
    /// New priority.
    #[arg(long)]
    pub priority: Option<String>,
    /// Add a label (repeatable).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Remove a label (repeatable).
    #[arg(long = "remove-label", value_name = "LABEL")]
    pub remove_labels: Vec<String>,
    /// Add an assignee (repeatable).
    #[arg(long = "assignee", value_name = "WHO")]
    pub assignees: Vec<String>,
    /// Remove an assignee (repeatable).
    #[arg(long = "unassign", value_name = "WHO")]
    pub unassign: Vec<String>,
    /// Add a blocked-by link (repeatable).
    #[arg(long = "blocked-by", value_name = "ID")]
    pub blocked_by: Vec<String>,
    /// Remove a blocked-by link (repeatable).
    #[arg(long = "unblock", value_name = "ID")]
    pub unblock: Vec<String>,
    /// Move the ticket to this list.
    #[arg(long, value_name = "LIST")]
    pub to: Option<String>,
    /// Also add this comment (`-` = stdin).
    #[arg(long, short = 'm', allow_hyphen_values = true)]
    pub comment: Option<String>,
    /// Read the comment from a file (`-` = stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "comment")]
    pub comment_file: Option<PathBuf>,
    /// Reattribute the ticket's creation to this identity (records an audit
    /// entry). Corrects a ticket created under the wrong/stomped identity.
    #[arg(long)]
    pub author: Option<String>,
}

/// `wipe ticket submit`
#[derive(Debug, Args)]
pub struct TicketSubmitArgs {
    /// Ticket ID.
    pub id: String,
    /// What was done (`-` = stdin).
    #[arg(
        long,
        short = 'm',
        visible_alias = "body",
        short_alias = 'b',
        allow_hyphen_values = true
    )]
    pub message: Option<String>,
    /// Read the message from a file (`-` = stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "message")]
    pub message_file: Option<PathBuf>,
    /// Something you verified (repeatable) - listed under "Tested".
    #[arg(long, value_name = "WHAT")]
    pub tested: Vec<String>,
    /// Something you did NOT verify (repeatable) - listed under "Not tested".
    #[arg(long, value_name = "WHAT")]
    pub untested: Vec<String>,
    /// Review list to move it to (default: `board.review_list`, else the list
    /// whose name contains "review").
    #[arg(long, value_name = "LIST")]
    pub to: Option<String>,
}

/// `wipe ticket approve`
#[derive(Debug, Args)]
pub struct TicketApproveArgs {
    /// Ticket ID.
    pub id: String,
    /// Optional comment (`-` = stdin).
    #[arg(
        long,
        short = 'm',
        visible_alias = "body",
        short_alias = 'b',
        allow_hyphen_values = true
    )]
    pub message: Option<String>,
    /// Approve even though some acceptance criteria are unchecked.
    #[arg(long)]
    pub force: bool,
}

/// `wipe ticket reject`
#[derive(Debug, Args)]
pub struct TicketRejectArgs {
    /// Ticket ID.
    pub id: String,
    /// Why it goes back - required (`-` = stdin).
    #[arg(
        long,
        short = 'm',
        visible_alias = "body",
        short_alias = 'b',
        allow_hyphen_values = true
    )]
    pub message: Option<String>,
    /// Read the reason from a file (`-` = stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "message")]
    pub message_file: Option<PathBuf>,
    /// List to send it back to (default: `board.rework_list`, else `todo`).
    #[arg(long, value_name = "LIST")]
    pub to: Option<String>,
}

/// `wipe ticket block` / `wipe ticket unblock`
#[derive(Debug, Args)]
pub struct TicketBlockArgs {
    /// The waiting ticket.
    pub id: String,
    /// The ticket it waits on (repeatable).
    #[arg(long = "by", value_name = "ID", required = true)]
    pub by: Vec<String>,
}

/// `wipe ticket list`
#[derive(Debug, Args)]
pub struct TicketListArgs {
    /// Only tickets on this list (repeatable).
    #[arg(short = 'l', long = "list", value_name = "LIST")]
    pub lists: Vec<String>,
    /// Skip tickets on this list (repeatable), e.g. `--exclude-list done`.
    #[arg(long = "exclude-list", value_name = "LIST")]
    pub exclude_lists: Vec<String>,
    /// Only tickets carrying this label (repeatable; all must match).
    #[arg(long = "label", value_name = "LABEL")]
    pub labels: Vec<String>,
    /// Only tickets assigned to this identity (`me` = yours).
    #[arg(long, value_name = "WHO")]
    pub assignee: Option<String>,
    /// Only tickets changed since: RFC-3339, a date (2026-09-01), or a span
    /// ago (`30m`, `12h`, `2d`, `1w`).
    #[arg(long, value_name = "WHEN")]
    pub since: Option<String>,
    /// Only tickets not waiting on an open blocker ("what can be done next?").
    #[arg(long, conflicts_with = "blocked")]
    pub ready: bool,
    /// Only tickets waiting on an open blocker.
    #[arg(long)]
    pub blocked: bool,
    /// Only tickets matching this search (same syntax as `wipe search`),
    /// best match first.
    #[arg(long, short = 'q', value_name = "QUERY")]
    pub search: Option<String>,
    /// JSON: comma-separated fields per row. Default: id, title, list, labels,
    /// assignees, priority, blocked_by. Also: comments, checklist, criteria,
    /// updated, last_by, created, body.
    #[arg(long, value_delimiter = ',', value_name = "FIELDS")]
    pub fields: Vec<String>,
    /// Full ticket objects (body, comments, activity) instead of compact rows.
    #[arg(long, conflicts_with = "fields")]
    pub full: bool,
    /// Cap the number of rows [default: 100; 0 = no cap].
    #[arg(long)]
    pub limit: Option<usize>,
}

/// `wipe comment ...`
#[derive(Debug, Subcommand)]
pub enum CommentCmd {
    /// Add a comment to a ticket.
    Add {
        /// Ticket ID.
        ticket: String,
        /// Comment body (or pass it with --body / --body-file).
        #[arg(value_name = "BODY")]
        body_pos: Option<String>,
        /// Comment body (Markdown allowed). `-` reads it from stdin.
        #[arg(long, short, allow_hyphen_values = true, conflicts_with = "body_pos")]
        body: Option<String>,
        /// Read the body from a file (`-` = stdin).
        #[arg(long, value_name = "PATH", conflicts_with_all = ["body", "body_pos"])]
        body_file: Option<PathBuf>,
        /// Author this one comment as a different identity.
        #[arg(long)]
        author: Option<String>,
    },
    /// List a ticket's comments (each clipped to 1000 characters).
    List {
        /// Ticket ID.
        ticket: String,
        /// Don't clip long comments.
        #[arg(long)]
        full: bool,
    },
    /// Remove a comment from a ticket.
    Remove {
        /// Ticket ID.
        ticket: String,
        /// Comment ID (e.g. c-1).
        comment: String,
    },
    /// Edit a comment's body (stamps it as edited).
    Edit {
        /// Ticket ID.
        ticket: String,
        /// Comment ID (e.g. c-1).
        comment: String,
        /// New body (Markdown allowed). `-` reads it from stdin.
        #[arg(long, short, allow_hyphen_values = true)]
        body: Option<String>,
        /// Read the new body from a file (`-` = stdin).
        #[arg(long, value_name = "PATH", conflicts_with = "body")]
        body_file: Option<PathBuf>,
    },
    /// Reattribute a comment to another identity, recording an audit entry (for
    /// correcting a comment written under the wrong/stomped identity).
    Reattribute {
        /// Ticket ID.
        ticket: String,
        /// Comment ID (e.g. c-1).
        comment: String,
        /// The identity to attribute the comment to.
        #[arg(long)]
        to: String,
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
    /// Apply one or more labels to a ticket.
    Assign {
        /// Ticket ID.
        ticket: String,
        /// Label names.
        #[arg(required = true)]
        names: Vec<String>,
    },
    /// Remove one or more labels from a ticket.
    Remove {
        /// Ticket ID.
        ticket: String,
        /// Label names.
        #[arg(required = true)]
        names: Vec<String>,
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
        /// Don't clip long posts (each is cut at 1500 characters by default).
        #[arg(long)]
        full: bool,
    },
    /// List threads, newest first.
    List {
        /// Only threads whose root carries this label.
        #[arg(long)]
        label: Option<String>,
        /// Only threads whose root was posted by this author (substring).
        #[arg(long)]
        author: Option<String>,
        /// Cap the number of threads shown [default: 25; 0 = no cap].
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Search posts by regex pattern and/or filters.
    Search(ForumSearchArgs),
    /// Edit a post's body and/or reattribute it (at least one is required).
    Edit {
        /// Post ID.
        id: String,
        /// New body (Markdown allowed). `-` reads it from stdin.
        #[arg(long, short, allow_hyphen_values = true)]
        body: Option<String>,
        /// Read the new body from a file (`-` = stdin).
        #[arg(long, value_name = "PATH", conflicts_with = "body")]
        body_file: Option<PathBuf>,
        /// Reattribute the post to this identity, recording the correction (for
        /// a post written under the wrong/stomped identity).
        #[arg(long)]
        author: Option<String>,
    },
    /// Delete a post and its entire subtree (root deletes the whole thread).
    Delete {
        /// Post or thread ID.
        id: String,
        /// Required to actually delete (subtree deletion is irreversible).
        #[arg(long)]
        yes: bool,
    },
    /// A compact, size-bounded Markdown digest of the pinned threads, for loading
    /// into an agent's context at session start (e.g. from CLAUDE.md or a hook).
    Digest(ForumDigestArgs),
    /// Pin a thread so it appears in `wipe forum digest` (adds the `pinned` label).
    Pin {
        /// Thread ID (e.g. F-1).
        id: String,
    },
    /// Unpin a thread.
    Unpin {
        /// Thread ID (e.g. F-1).
        id: String,
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
    /// Message body (Markdown allowed). `-` reads it from stdin.
    #[arg(long, short, allow_hyphen_values = true)]
    pub body: Option<String>,
    /// Read the body from a file (`-` = stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "body")]
    pub body_file: Option<PathBuf>,
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
    /// Reply body (Markdown allowed). `-` reads it from stdin.
    #[arg(long, short, allow_hyphen_values = true)]
    pub body: Option<String>,
    /// Read the body from a file (`-` = stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "body")]
    pub body_file: Option<PathBuf>,
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
    /// Cap the number of results (`0` = no cap).
    #[arg(long, default_value = "50")]
    pub limit: usize,
    /// Make the pattern case-sensitive (default: case-insensitive).
    #[arg(long = "case-sensitive")]
    pub case_sensitive: bool,
}

/// `wipe forum digest`
#[derive(Debug, Args)]
pub struct ForumDigestArgs {
    /// Digest threads carrying this label instead of `pinned`.
    #[arg(long, default_value = "pinned")]
    pub label: String,
    /// Upper bound on the digest size in bytes (each thread is truncated to fit).
    #[arg(long, default_value = "4000")]
    pub max_bytes: usize,
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
    /// Where the UI is reachable: `lan` (all local networks, the default),
    /// `local` (this machine only), `tailscale` (this machine + your tailnet), or
    /// `proxy` (behind a reverse proxy). Overrides `daemon.expose`.
    #[arg(long, value_name = "MODE", conflicts_with_all = ["local", "tailscale"])]
    pub expose: Option<String>,
    /// Shorthand for `--expose local`: only this machine can connect.
    #[arg(long, conflicts_with = "tailscale")]
    pub local: bool,
    /// Shorthand for `--expose tailscale`: reachable over your tailnet only.
    #[arg(long)]
    pub tailscale: bool,
    /// Bind this exact address instead (e.g. `192.168.1.20` or `::`). Remote
    /// clients still need the access token.
    #[arg(long, value_name = "ADDR")]
    pub host: Option<String>,
    /// Don't print the QR code for opening the board on a phone.
    #[arg(long)]
    pub no_qr: bool,
    /// Open the UI in a browser once started.
    #[arg(long)]
    pub open: bool,
    /// Auto-stop after N seconds with no viewers (0 = never; overrides settings).
    #[arg(long, value_name = "SECS")]
    pub idle: Option<u64>,
}

/// `wipe tray`
#[derive(Debug, Args)]
pub struct TrayArgs {
    /// Stay attached to this terminal instead of moving to the background.
    #[arg(long)]
    pub foreground: bool,
    /// Port to serve on (default: your global default, else 6737).
    #[arg(long)]
    pub port: Option<u16>,
}

/// `wipe config ...`
#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Show all settings.
    Show,
    /// Get a setting by key (daemon.port, daemon.expose, board.name,
    /// board.autocommit, board.review_list, board.done_list, board.rework_list).
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
