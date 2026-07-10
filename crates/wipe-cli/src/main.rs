//! The `wipe` binary: parse arguments, dispatch to a command, and translate any
//! error into a clean exit code (and, in `--json` mode, a machine-readable error
//! object on stdout).

mod args;
mod autostart;
mod commands;
mod first_run;
mod forum_cmd;
mod identity;
mod onboard;
mod output;
mod skills;
mod update_check;

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

use args::{Cli, Command};
use output::{emit_error, Out};

fn main() -> ExitCode {
    let cli = Cli::parse();

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
    if let Some(id) = cli.agentid.as_deref() {
        // Make the agent visible in the board's identity list (best-effort).
        identity::ensure_registered(id, None, true);
    }

    // On the very first interactive run of a fresh install, offer the guided global
    // setup. Skipped for the commands that either *are* that setup (`onboard`) or run
    // their own wizard (`init`), and for `completions` (shell-eval'd, must stay quiet).
    let may_offer_onboarding = !matches!(
        cli.command,
        Command::Onboard(_) | Command::Init(_) | Command::Completions { .. }
    ) && first_run::should_offer(cli.json);

    let out = Out::new(cli.json);

    if may_offer_onboarding && first_run::offer() {
        if let Err(e) = commands::onboard(&out, args::OnboardArgs { yes: false }) {
            eprintln!("wipe: guided setup did not complete: {e:#}");
        }
    }

    let result = dispatch(&out, cli.command);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            emit_error(cli.json, &format!("{e:#}"));
            ExitCode::FAILURE
        }
    }
}

fn dispatch(out: &Out, command: Command) -> anyhow::Result<()> {
    match command {
        Command::Init(a) => commands::init(out, a),
        Command::Onboard(a) => commands::onboard(out, a),
        Command::Identity(c) => commands::identity(out, c),
        Command::Scan(a) => commands::scan(out, a),
        Command::Status => commands::status(out),
        Command::Board(c) => commands::board(out, c),
        Command::List(c) => commands::list(out, c),
        Command::Ticket(c) => commands::ticket(out, c),
        Command::Comment(c) => commands::comment(out, c),
        Command::Label(c) => commands::label(out, c),
        Command::Media(c) => commands::media(out, c),
        Command::Forum(c) => forum_cmd::run(out, c),
        Command::Serve(a) => commands::serve(out, a),
        Command::Config { global, cmd } => commands::config(out, global, cmd),
        Command::Doctor => commands::doctor(out),
        Command::Skill { cmd } => commands::skill(out, cmd),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "wipe", &mut std::io::stdout());
            Ok(())
        }
    }
}
