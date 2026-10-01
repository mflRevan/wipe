//! Ticket-id ergonomics at the CLI boundary: every ticket argument is rewritten
//! to its canonical id before a command runs (so `wipe ticket show t2a`,
//! `T-2A`, `T-02A` - or a pre-translation `T-42` - all act on, and print,
//! `T-02A`), plus the one-time offer to translate a legacy decimal board.

use std::io::IsTerminal;

use wipe_core::model::IdFormat;
use wipe_core::Store;

use crate::args::*;

/// Replace `id` with the canonical form when it resolves; otherwise leave it
/// untouched so the command reports the usual "not found" error.
fn fix(s: &Store, id: &mut String) {
    if let Ok(c) = s.resolve_ticket_id(id) {
        *id = c;
    }
}

fn fix_all(s: &Store, ids: &mut [String]) {
    for id in ids {
        fix(s, id);
    }
}

/// Canonicalize the ticket ids carried by `cmd` (no-op outside a board).
pub fn canonicalize(cmd: &mut Command) {
    let Ok(s) = Store::discover(".") else {
        return;
    };
    let s = &s;
    match cmd {
        Command::Ticket(t) => match t {
            TicketCmd::Create(a) => fix_all(s, &mut a.blocked_by),
            TicketCmd::Show(a) => fix(s, &mut a.id),
            TicketCmd::Edit(a) => {
                fix(s, &mut a.id);
                fix_all(s, &mut a.blocked_by);
                fix_all(s, &mut a.unblock);
            }
            TicketCmd::Submit(a) => fix(s, &mut a.id),
            TicketCmd::Approve(a) => fix(s, &mut a.id),
            TicketCmd::Reject(a) => fix(s, &mut a.id),
            TicketCmd::Block(a) | TicketCmd::Unblock(a) => {
                fix(s, &mut a.id);
                fix_all(s, &mut a.by);
            }
            TicketCmd::Move { id, .. }
            | TicketCmd::Assign { id, .. }
            | TicketCmd::Close { id }
            | TicketCmd::Reopen { id }
            | TicketCmd::Delete { id, .. }
            | TicketCmd::Duplicate { id } => fix(s, id),
            TicketCmd::List(_) => {}
        },
        Command::Comment(c) => match c {
            CommentCmd::Add { ticket, .. }
            | CommentCmd::List { ticket }
            | CommentCmd::Remove { ticket, .. }
            | CommentCmd::Edit { ticket, .. }
            | CommentCmd::Reattribute { ticket, .. } => fix(s, ticket),
        },
        Command::Checklist(c) | Command::Criteria(c) => match c {
            ChecklistCmd::Add { ticket, .. }
            | ChecklistCmd::List { ticket }
            | ChecklistCmd::Check { ticket, .. }
            | ChecklistCmd::Uncheck { ticket, .. }
            | ChecklistCmd::Toggle { ticket, .. }
            | ChecklistCmd::Edit { ticket, .. }
            | ChecklistCmd::Remove { ticket, .. }
            | ChecklistCmd::Move { ticket, .. } => fix(s, ticket),
        },
        Command::Label(LabelCmd::Assign { ticket, .. } | LabelCmd::Remove { ticket, .. }) => {
            fix(s, ticket)
        }
        Command::Media(
            MediaCmd::Add { ticket, .. }
            | MediaCmd::List { ticket }
            | MediaCmd::Remove { ticket, .. },
        ) => fix(s, ticket),
        Command::Commit(a) => {
            if let Some(t) = &mut a.target {
                fix(s, t);
            }
        }
        Command::Subscribe(a) | Command::Unsubscribe(a)
            if wipe_core::id::ticket_ref_digits(&a.target).is_some() =>
        {
            fix(s, &mut a.target)
        }
        _ => {}
    }
}

/// On the first CLI run against a legacy decimal board (per machine and board),
/// offer to translate it to hex ids. Interactive terminals get a y/N question;
/// agents and scripts (non-TTY or `--json`) get a one-line stderr notice instead,
/// never a prompt that would block them. Either way it is asked only once; the
/// explicit command stays available: `wipe board translate-ids`.
pub fn offer_translation(json: bool, cmd: &Command) {
    if matches!(
        cmd,
        Command::Board(BoardCmd::TranslateIds { .. })
            | Command::Completions { .. }
            | Command::Init(_)
            | Command::Serve(_)
    ) {
        return;
    }
    let Ok(s) = Store::discover(".") else {
        return;
    };
    if s.load_board()
        .map(|b| b.ids != IdFormat::Decimal)
        .unwrap_or(true)
    {
        return;
    }
    let marker = s.cache_dir().join("ids-translation-offered");
    if marker.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(s.cache_dir());
    let _ = std::fs::write(&marker, chrono::Utc::now().to_rfc3339());

    let interactive = !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    if !interactive {
        crate::output::hint(
            "this board still uses the old decimal ticket ids (T-23). `wipe board translate-ids` \
             converts it to the new fixed-width hex ids (T-017); old ids keep working as aliases",
        );
        return;
    }
    eprintln!(
        "This board still uses the old decimal ticket ids (T-23). New boards use fixed-width\n\
         hex ids (T-001, T-2AF) that are quicker to type and search. Translating renames the\n\
         ticket files and rewrites references; old ids keep working as aliases.\n\
         Commit or merge open branches first, and have collaborators pull right after."
    );
    let yes = inquire::Confirm::new("Translate this board's ticket ids now?")
        .with_default(false)
        .with_help_message("asked once - later: `wipe board translate-ids`")
        .prompt()
        .unwrap_or(false);
    if yes {
        match translate(&s) {
            Ok(n) => eprintln!("translated {n} ticket ids to the hex format"),
            Err(e) => eprintln!("translation failed: {e:#}"),
        }
    }
}

/// Run the translation under the board's write lock. Returns how many ids changed.
pub fn translate(s: &Store) -> anyhow::Result<usize> {
    let _lock = s.lock()?;
    Ok(wipe_core::translate::translate_ids(s, chrono::Utc::now())?.len())
}
