//! The `wipe` binary: parse arguments, dispatch to a command, and translate any
//! error into a clean exit code (and, in `--json` mode, a machine-readable error
//! object on stdout).

mod args;
mod commands;
mod identity;
mod output;

use std::process::ExitCode;

use clap::Parser;

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

    let out = Out::new(cli.json);
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
        Command::Status => commands::status(out),
        Command::Board(c) => commands::board(out, c),
        Command::List(c) => commands::list(out, c),
        Command::Ticket(c) => commands::ticket(out, c),
        Command::Comment(c) => commands::comment(out, c),
        Command::Label(c) => commands::label(out, c),
        Command::Media(c) => commands::media(out, c),
        Command::Serve(a) => commands::serve(out, a),
        Command::Config(c) => commands::config(out, c),
        Command::Doctor => commands::doctor(out),
        Command::Skill => commands::skill(out),
    }
}
