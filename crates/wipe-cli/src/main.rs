//! The `wipe` binary: parse arguments, dispatch to a command, and translate any
//! error into a clean exit code (and, in `--json` mode, a machine-readable error
//! object on stdout).

mod args;
mod autostart;
mod commands;
mod first_run;
mod forum_cmd;
mod identity;
mod ids;
mod input;
mod onboard;
mod output;
mod skills;
mod tray;
mod update_check;
mod view;

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

use args::{Cli, Command};
use output::{emit_error, Out};

fn main() -> ExitCode {
    let mut cli = Cli::parse();

    // Honor `-C/--cwd` by switching directories before anything touches the board.
    if let Some(dir) = &cli.cwd {
        if let Err(e) = std::env::set_current_dir(dir) {
            emit_error(cli.json, &format!("cannot enter {}: {e}", dir.display()));
            return ExitCode::FAILURE;
        }
    }

    // Once a day, quietly note if a newer version is published (stderr only, so
    // `--json` stdout stays clean). Skipped for `completions`, whose output is
    // eval'd by the shell and should stay fast and side-effect-free.
    if !matches!(cli.command, Command::Completions { .. }) {
        update_check::run(env!("CARGO_PKG_VERSION"));
    }

    // Record the global --agentid override before any command resolves an author.
    identity::set_override(cli.agentid.clone());

    // On the very first interactive run of a fresh install, offer the guided global
    // setup. Skipped for the commands that either *are* that setup (`onboard`) or run
    // their own wizard (`init`), and for `completions` (shell-eval'd, must stay quiet).
    let may_offer_onboarding = !matches!(
        cli.command,
        Command::Onboard(_) | Command::Init(_) | Command::Completions { .. }
    ) && first_run::should_offer(cli.json);

    let out = Out::new(cli.json, cli.echo, cli.pretty);

    if may_offer_onboarding && first_run::offer() {
        if let Err(e) = commands::onboard(&out, args::OnboardArgs { yes: false }) {
            eprintln!("wipe: guided setup did not complete: {e:#}");
        }
    }

    // Legacy decimal boards: offer the hex-id translation once; then accept any
    // spelling of a ticket id (`t2a`, `T-02A`, a pre-translation `T-42`).
    ids::offer_translation(cli.json, &cli.command);
    ids::canonicalize(&mut cli.command);

    let write = is_write(&cli.command);
    // Every write needs a chosen identity - there is no default to fall back to.
    // Refuse up front, before anything is read or written, with guidance.
    let mut _lock = None;
    if write {
        let Some(who) = identity::resolve_opt(actor_override(&cli.command)) else {
            emit_error(cli.json, &identity::missing_identity_message());
            return ExitCode::FAILURE;
        };
        // Serialize with every other writer of this board (other agents, the UI).
        if let Ok(s) = wipe_core::Store::discover(".") {
            match s.lock() {
                Ok(l) => _lock = Some(l),
                Err(e) => {
                    emit_error(cli.json, &format!("cannot take the board write lock: {e}"));
                    return ExitCode::FAILURE;
                }
            }
        }
        // An agent named by --agentid / $WIPE_AGENT shows up in the board's
        // identity list like any other author (best-effort, insert-only).
        if cli.agentid.is_some() || identity::agent_env().is_some() {
            identity::ensure_registered(&who, None, true);
        }
    }

    let is_commit = matches!(cli.command, Command::Commit(_));
    let result = dispatch(&out, cli.command);

    match result {
        Ok(()) => {
            if write && !is_commit {
                after_write(&out);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            emit_error(cli.json, &format!("{e:#}"));
            ExitCode::FAILURE
        }
    }
}

/// Whether a command writes to the board (and so needs an identity, takes the
/// write lock, and is eligible for `board.autocommit`). Reads - including
/// identity-scoped ones like `inbox` - are never gated.
fn is_write(c: &Command) -> bool {
    use args::*;
    match c {
        Command::Board(b) => !matches!(b, BoardCmd::Show),
        Command::List(l) => !matches!(l, ListCmd::Show),
        Command::Ticket(t) => !matches!(t, TicketCmd::Show(_) | TicketCmd::List(_)),
        Command::Comment(x) => !matches!(x, CommentCmd::List { .. }),
        Command::Checklist(x) | Command::Criteria(x) => !matches!(x, ChecklistCmd::List { .. }),
        Command::Label(x) => !matches!(x, LabelCmd::List),
        Command::Media(x) => !matches!(x, MediaCmd::List { .. }),
        Command::Forum(x) => matches!(
            x,
            ForumCmd::Post(_)
                | ForumCmd::Reply(_)
                | ForumCmd::Edit { .. }
                | ForumCmd::Delete { .. }
                | ForumCmd::Pin { .. }
                | ForumCmd::Unpin { .. }
        ),
        Command::Subscribe(_) | Command::Unsubscribe(_) | Command::Commit(_) => true,
        Command::Trash { cmd } => !matches!(cmd, TrashCmd::List),
        Command::Config { global, cmd } => !global && matches!(cmd, ConfigCmd::Set { .. }),
        _ => false,
    }
}

/// The per-command *actor* override, if the command carries one. Reattribution
/// targets (e.g. `ticket edit --author`, `comment reattribute --to`) are NOT actor
/// overrides - those writes are still performed by the session identity.
fn actor_override(c: &Command) -> Option<&str> {
    use args::{CommentCmd, ForumCmd};
    match c {
        Command::Comment(CommentCmd::Add { author, .. }) => author.as_deref(),
        Command::Forum(ForumCmd::Post(a)) => a.author.as_deref(),
        Command::Forum(ForumCmd::Reply(a)) => a.author.as_deref(),
        Command::Subscribe(a) | Command::Unsubscribe(a) => a.author.as_deref(),
        Command::Commit(a) => a.author.as_deref(),
        _ => None,
    }
}

/// After a successful write: auto-commit `.wipe/` if the board opts in; else -
/// until `wipe commit` has been used once on this machine - a one-line stderr
/// hint that the board changed and how to record it. Best-effort; stdout (and so
/// the `--json` contract) is never touched.
fn after_write(out: &Out) {
    let Ok(s) = wipe_core::Store::discover(".") else {
        return;
    };
    let Ok(settings) = s.load_settings() else {
        return;
    };
    if !wipe_core::git::is_repo(s.root()) {
        return;
    }
    if settings.autocommit {
        let Some(who) = identity::resolve_opt(None) else {
            return;
        };
        if let Ok(Some(h)) = wipe_core::ops::commit_board(&s, None, None, &who) {
            if !out.json {
                eprintln!("  auto-committed {h}");
            }
        }
    } else if wipe_core::GlobalConfig::load().commit_hint_seen != Some(true) {
        output::hint(
            "board changed - `wipe commit` records it as one wipe-attributed commit (and keeps \
             .wipe/ out of your own commits); `wipe config set board.autocommit true` commits \
             after every write",
        );
    }
}

fn dispatch(out: &Out, command: Command) -> anyhow::Result<()> {
    match command {
        Command::Init(a) => commands::init(out, a),
        Command::Onboard(a) => commands::onboard(out, a),
        Command::Identity(c) => commands::identity(out, c),
        Command::Scan(a) => commands::scan(out, a),
        Command::Status(a) => commands::status(out, a),
        Command::Board(c) => commands::board(out, c),
        Command::List(c) => commands::list(out, c),
        Command::Ticket(c) => commands::ticket(out, c),
        Command::Comment(c) => commands::comment(out, c),
        Command::Checklist(c) => commands::checklist(out, c),
        Command::Criteria(c) => commands::criteria(out, c),
        Command::Label(c) => commands::label(out, c),
        Command::Media(c) => commands::media(out, c),
        Command::Forum(c) => forum_cmd::run(out, c),
        Command::Serve(a) => commands::serve(out, a),
        Command::Tray(a) => tray::run(a),
        Command::Config { global, cmd } => commands::config(out, global, cmd),
        Command::Subscribe(a) => commands::subscribe(out, a, false),
        Command::Unsubscribe(a) => commands::subscribe(out, a, true),
        Command::Subscriptions { author } => commands::subscriptions(out, author),
        Command::Inbox(a) => commands::inbox(out, a),
        Command::Trash { cmd } => commands::trash(out, cmd),
        Command::Commit(a) => commands::commit(out, a),
        Command::Doctor => commands::doctor(out),
        Command::Skill { cmd } => commands::skill(out, cmd),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "wipe", &mut std::io::stdout());
            Ok(())
        }
    }
}
