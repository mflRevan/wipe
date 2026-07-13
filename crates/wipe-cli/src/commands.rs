//! Implementations of every `wipe` subcommand. Each returns `anyhow::Result<()>`
//! and prints through [`Out`], so the caller only has to map errors to exit codes.

use std::io::IsTerminal;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

use wipe_core::model::{Exposure, IdentityKind, Starter};
use wipe_core::ops::{self, NewTicket, TicketPatch};
use wipe_core::{registry, vcs, GlobalConfig, Store};

use crate::args::*;
use crate::autostart;
use crate::identity;
use crate::onboard;
use crate::output::{dim, id_style, Out};
use crate::skills;

/// The embedded agent SKILL guide, printed by `wipe skill`.
const SKILL: &str = include_str!("../skills/SKILL.md");

/// Open the board for the current directory.
fn store() -> Result<Store> {
    Store::discover(".").map_err(Into::into)
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("model is serializable")
}

/// Strip Windows' `\\?\` verbatim prefix so displayed paths read naturally.
fn clean_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

// ---------------------------------------------------------------------------

/// `wipe init` - a guided wizard by default; non-interactive with `--yes`,
/// `--json`, or when not attached to a terminal.
pub fn init(out: &Out, args: InitArgs) -> Result<()> {
    std::fs::create_dir_all(&args.path)
        .with_context(|| format!("creating {}", args.path.display()))?;
    let default_name = match &args.name {
        Some(n) => n.clone(),
        None => std::fs::canonicalize(&args.path)?
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("board")
            .to_string(),
    };

    let g = GlobalConfig::load();
    let starter_flag = args
        .starter
        .as_deref()
        .map(onboard::parse_starter)
        .transpose()?;

    let interactive =
        !args.yes && !out.json && std::io::stdin().is_terminal() && std::io::stdout().is_terminal();

    let plan = if interactive {
        let default_starter = starter_flag.or(g.starter).unwrap_or_default();
        onboard::wizard(&default_name, default_starter, &g)?
    } else {
        onboard::non_interactive(default_name.clone(), starter_flag, &g)
    };

    let store = Store::init_with(&args.path, &plan.name, Utc::now(), plan.starter)?;

    // Persist the chosen daemon settings into the board's settings.json.
    let mut settings = store.load_settings()?;
    settings.daemon.port = plan.port;
    settings.daemon.expose = plan.expose;
    settings.daemon.autoserve = plan.autoserve;
    settings.daemon.idle_timeout_secs = plan.idle_timeout_secs;
    // The board's shared fallback author must be GENERIC, not the creator's personal
    // identity: settings.json is git-tracked, so baking a specific person in would
    // misattribute every collaborator whose VCS reports no user. Each user's own VCS
    // identity is resolved at runtime and takes precedence over this fallback.
    settings.default_author = Some(
        g.default_identity
            .clone()
            .unwrap_or_else(|| "human".to_string()),
    );
    store.save_settings(&settings)?;

    // Record this board in the registry so `wipe serve` from anywhere lists it.
    registry::register(store.root());

    // Install the agent skill if the wizard asked for it (best-effort).
    let mut skill_path: Option<String> = None;
    if let Some(choice) = plan.skill {
        let sargs = SkillInstallArgs {
            target: Some(choice.target.slug().to_string()),
            global: choice.global,
            dir: None,
            force: false,
        };
        match skills::plan(&sargs).and_then(|p| {
            skills::install(&p, SKILL, false)?;
            Ok(p)
        }) {
            Ok(p) => skill_path = Some(clean_path(&p.file)),
            Err(e) if !out.json => out.line(format!("  (skill not installed: {e})")),
            Err(_) => {}
        }
    }

    // Remember interactive choices as global defaults for next time.
    if interactive {
        let _ = onboard::remember(&plan);
    }

    let path = clean_path(&store.wipe_dir());
    out.ok(
        format!("initialized wipe board '{}' at {path}", plan.name),
        json!({
            "ok": true,
            "name": plan.name,
            "path": path,
            "starter": starter_slug(plan.starter),
            "port": plan.port,
            "autoserve": plan.autoserve,
            "skill": skill_path,
        }),
    );
    if !out.json {
        if let Some(p) = &skill_path {
            println!("  installed agent skill at {p}");
        }
        println!("\n  next steps:");
        println!("    wipe serve                      open the board UI");
        println!("    wipe ticket create -t \"...\"      add your first card");
        if plan.skill.is_none() {
            println!("    wipe skill install              teach coding agents to drive this board");
        }
    }
    Ok(())
}

/// `wipe onboard` - a guided, machine-wide setup that records your global
/// defaults (port, exposure, autoserve, login autostart, starter, skill, styling).
pub fn onboard(out: &Out, args: OnboardArgs) -> Result<()> {
    let g = GlobalConfig::load();
    let interactive =
        !args.yes && !out.json && std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if !interactive {
        // Non-interactive: show the current config rather than prompting.
        return config_global(out, ConfigCmd::Show);
    }

    let updated = onboard::global_wizard(&g)?;

    // Apply the login-autostart toggle against the OS's real state (best-effort).
    let want = updated.autostart.unwrap_or(false);
    let mut autostart_note: Option<String> = None;
    if want && !autostart::is_enabled() {
        match autostart::enable() {
            Ok(note) => autostart_note = Some(note),
            Err(e) => out.line(format!("  (autostart not enabled: {e})")),
        }
    } else if !want && autostart::is_enabled() {
        match autostart::disable() {
            Ok(note) => autostart_note = Some(note),
            Err(e) => out.line(format!("  (autostart not disabled: {e})")),
        }
    }

    updated.save().context("saving global config")?;
    let path = GlobalConfig::path()
        .map(|p| clean_path(&p))
        .unwrap_or_else(|| "(unavailable)".into());
    out.ok(
        "saved your global wipe preferences",
        json!({ "ok": true, "path": path, "autostart": want }),
    );
    if !out.json {
        if let Some(n) = autostart_note {
            println!("  {n}");
        }
        println!("  config: {path}");
        println!("\n  run `wipe serve` to open the UI, or `wipe init` to start a board.");
    }
    Ok(())
}

/// `wipe identity ...` - see and manage who actions are attributed to.
pub fn identity(out: &Out, cmd: IdentityCmd) -> Result<()> {
    match cmd {
        IdentityCmd::List => {
            let store = Store::discover(".").ok();
            let ids = match &store {
                Some(s) => ops::list_identities(s)?,
                None => Vec::new(),
            };
            let active = crate::identity::active();
            if out.json {
                out.json_value(&json!({
                    "active": active,
                    "default": crate::identity::effective_default(),
                    "identities": ids.iter().map(to_value).collect::<Vec<_>>(),
                }));
            } else {
                if ids.is_empty() {
                    out.line("no identities yet - `wipe identity use <id>` to create one");
                }
                for i in &ids {
                    let mark = if active.as_deref() == Some(&i.id) {
                        "*"
                    } else {
                        " "
                    };
                    let kind = match i.kind {
                        IdentityKind::Agent => "agent",
                        IdentityKind::Human => "human",
                    };
                    println!(
                        "{mark} {}  {}  {}",
                        id_style(&i.id),
                        i.display_name,
                        dim(kind)
                    );
                }
                if let Some(a) = &active {
                    if !ids.iter().any(|i| &i.id == a) {
                        println!("* {}  {}", id_style(a), dim("(session)"));
                    }
                }
            }
        }
        IdentityCmd::Use(a) => {
            if a.id.trim().is_empty() {
                bail!("identity id cannot be empty");
            }
            let is_email = a.id.contains('@');
            let agent = a.agent || (!a.human && !is_email);
            let name = a.name.clone().unwrap_or_else(|| a.id.clone());
            if let Ok(store) = Store::discover(".") {
                let kind = if agent {
                    IdentityKind::Agent
                } else {
                    IdentityKind::Human
                };
                let _ = ops::upsert_identity(&store, &a.id, &name, Some(kind));
            }
            crate::identity::set_active(&a.id)?;
            let hint = crate::identity::export_hint(&a.id);
            out.ok(
                format!("actions in this session are now by {}", a.id),
                json!({ "ok": true, "id": a.id, "name": name, "agent": agent, "export": hint }),
            );
            if !out.json {
                println!("  to pin it across tool-spawned shells, run:");
                println!("    {hint}");
            }
        }
        IdentityCmd::Whoami => {
            let who = crate::identity::resolve(None);
            let src = crate::identity::source(None);
            out.ok(
                format!("{who}  ({src})"),
                json!({ "identity": who, "source": src }),
            );
        }
        IdentityCmd::Clear => {
            let cleared = crate::identity::clear_active()?;
            out.ok(
                if cleared {
                    "cleared this session's identity"
                } else {
                    "no session identity was set"
                },
                json!({ "ok": true, "cleared": cleared }),
            );
        }
    }
    Ok(())
}

/// Scan roots to search for boards: explicit paths, else configured roots, else home.
fn configured_scan_roots() -> Vec<std::path::PathBuf> {
    let g = GlobalConfig::load();
    match g.scan_roots {
        Some(roots) if !roots.is_empty() => {
            roots.into_iter().map(std::path::PathBuf::from).collect()
        }
        _ => registry::default_scan_roots(),
    }
}

/// `wipe scan` - discover boards on disk and add them to the local registry.
pub fn scan(out: &Out, args: ScanArgs) -> Result<()> {
    let roots = if args.paths.is_empty() {
        configured_scan_roots()
    } else {
        args.paths.clone()
    };
    registry::prune();
    let found = registry::scan(&roots, args.depth);
    let all = registry::list();
    if out.json {
        out.json_value(&json!({
            "found": found,
            "total": all.len(),
            "projects": all.iter().map(to_value).collect::<Vec<_>>(),
        }));
    } else if found.is_empty() {
        out.line(format!("no new boards found ({} already known)", all.len()));
    } else {
        println!("found {} new board(s):", found.len());
        for p in &found {
            println!("  {}", clean_path(std::path::Path::new(p)));
        }
    }
    Ok(())
}

/// Machine slug for a starter mode.
fn starter_slug(s: Starter) -> &'static str {
    match s {
        Starter::Standard => "standard",
        Starter::ListsOnly => "lists",
        Starter::Empty => "empty",
    }
}

/// `wipe status`
pub fn status(out: &Out) -> Result<()> {
    let s = store()?;
    let (board, view) = ops::board_view(&s)?;
    if out.json {
        let lists: Vec<Value> = view
            .iter()
            .map(|(list_id, tickets)| {
                json!({ "list": list_id, "tickets": tickets.iter().map(to_value).collect::<Vec<_>>() })
            })
            .collect();
        out.json_value(&json!({ "board": board.name, "lists": lists }));
        return Ok(());
    }
    println!(
        "{}  {}",
        board.name.as_str(),
        dim(&format!("({} lists)", board.lists.len()))
    );
    for (list, tickets) in &view {
        let name = board.list(list).map(|l| l.name.as_str()).unwrap_or(list);
        println!("\n{} {}", name, dim(&format!("[{}]", tickets.len())));
        for t in tickets {
            println!("  {}  {}", id_style(&t.id), t.title);
        }
    }
    Ok(())
}

/// `wipe board ...`
pub fn board(out: &Out, cmd: BoardCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        BoardCmd::Show => {
            let b = s.load_board()?;
            out.ok(
                format!("board '{}' ({} lists)", b.name, b.lists.len()),
                to_value(&b),
            );
        }
        BoardCmd::Rename { name } => {
            let mut b = s.load_board()?;
            b.name = name.clone();
            b.updated = Utc::now();
            s.save_board(&b)?;
            out.ok(
                format!("renamed board to '{name}'"),
                json!({ "ok": true, "name": name }),
            );
        }
    }
    Ok(())
}

/// `wipe list ...`
pub fn list(out: &Out, cmd: ListCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        ListCmd::Show => {
            let b = s.load_board()?;
            if out.json {
                out.json_value(
                    &json!({ "lists": b.lists.iter().map(to_value).collect::<Vec<_>>() }),
                );
            } else {
                for l in &b.lists {
                    println!(
                        "{}  {}  {}",
                        id_style(&l.id),
                        l.name,
                        dim(&format!("[{}]", l.cards.len()))
                    );
                }
            }
        }
        ListCmd::Add { name } => {
            let id = ops::add_list(&s, &name, Utc::now())?;
            out.ok(
                format!("added list '{name}' ({id})"),
                json!({ "ok": true, "id": id, "name": name }),
            );
        }
        ListCmd::Rename { id, name } => {
            ops::rename_list(&s, &id, &name, Utc::now())?;
            out.ok(
                format!("renamed list {id} to '{name}'"),
                json!({ "ok": true, "id": id, "name": name }),
            );
        }
        ListCmd::Move { id, index } => {
            ops::move_list(&s, &id, index, Utc::now())?;
            out.ok(
                format!("moved list {id} to position {index}"),
                json!({ "ok": true, "id": id, "index": index }),
            );
        }
        ListCmd::Remove { id, force } => {
            ops::remove_list(&s, &id, force, Utc::now())?;
            out.ok(
                format!("removed list {id}"),
                json!({ "ok": true, "id": id }),
            );
        }
    }
    Ok(())
}

/// `wipe ticket ...`
pub fn ticket(out: &Out, cmd: TicketCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        TicketCmd::Create(a) => {
            let spec = NewTicket {
                title: a.title,
                body: a.body,
                priority: a.priority,
                list: a.list,
                labels: a.labels,
                assignees: a.assignees,
            };
            let t = ops::create_ticket(&s, spec, &identity::resolve(None), Utc::now())?;
            out.ok(format!("created {} - {}", t.id, t.title), to_value(&t));
        }
        TicketCmd::Show { id } => {
            let t = s.load_ticket(&id)?;
            let board = s.load_board()?;
            let list_id = board.locate_card(&id).map(|(l, _)| l);
            if out.json {
                let mut v = to_value(&t);
                v.as_object_mut()
                    .unwrap()
                    .insert("list".into(), json!(list_id));
                out.json_value(&v);
            } else {
                print_ticket_human(&t, list_id.as_deref());
            }
        }
        TicketCmd::Edit(a) => {
            let patch = TicketPatch {
                title: a.title,
                body: a.body,
                // Provided priority sets it; absent leaves it unchanged (CLI edit
                // has no "clear" form, matching prior behavior).
                priority: a.priority.map(Some),
                ..Default::default()
            };
            let t = ops::update_ticket(&s, &a.id, patch, &identity::resolve(None), Utc::now())?;
            out.ok(format!("updated {}", t.id), to_value(&t));
        }
        TicketCmd::Move { id, to, pos } => {
            ops::move_ticket(&s, &id, &to, pos, &identity::resolve(None), Utc::now())?;
            out.ok(
                format!("moved {id} to {to}"),
                json!({ "ok": true, "id": id, "list": to }),
            );
        }
        TicketCmd::Assign { id, who, remove } => {
            let mut t = s.load_ticket(&id)?;
            let mut assignees = t.assignees.clone();
            if remove {
                assignees.retain(|a| a != &who);
            } else if !assignees.contains(&who) {
                assignees.push(who.clone());
            }
            let patch = TicketPatch {
                assignees: Some(assignees),
                ..Default::default()
            };
            t = ops::update_ticket(&s, &id, patch, &identity::resolve(None), Utc::now())?;
            let verb = if remove { "unassigned" } else { "assigned" };
            out.ok(format!("{verb} {who} on {id}"), to_value(&t));
        }
        TicketCmd::Close { id } => {
            let board = s.load_board()?;
            let target = done_list(&board).ok_or_else(|| anyhow!("board has no lists"))?;
            ops::move_ticket(&s, &id, &target, None, &identity::resolve(None), Utc::now())?;
            out.ok(
                format!("closed {id} (moved to {target})"),
                json!({ "ok": true, "id": id, "list": target }),
            );
        }
        TicketCmd::Reopen { id } => {
            let board = s.load_board()?;
            let target = board
                .lists
                .first()
                .map(|l| l.id.clone())
                .ok_or_else(|| anyhow!("board has no lists"))?;
            ops::move_ticket(&s, &id, &target, None, &identity::resolve(None), Utc::now())?;
            out.ok(
                format!("reopened {id} (moved to {target})"),
                json!({ "ok": true, "id": id, "list": target }),
            );
        }
        TicketCmd::Delete { id, yes } => {
            if !yes {
                bail!("refusing to delete {id} without --yes");
            }
            ops::delete_ticket(&s, &id, Utc::now())?;
            out.ok(format!("deleted {id}"), json!({ "ok": true, "id": id }));
        }
        TicketCmd::List(a) => {
            let (board, view) = ops::board_view(&s)?;
            let mut rows: Vec<(String, wipe_core::model::Ticket)> = Vec::new();
            for (list_id, tickets) in view {
                if let Some(f) = &a.list {
                    if &list_id != f {
                        continue;
                    }
                }
                for t in tickets {
                    if let Some(l) = &a.label {
                        if !t.labels.contains(l) {
                            continue;
                        }
                    }
                    rows.push((list_id.clone(), t));
                }
            }
            if out.json {
                let arr: Vec<Value> = rows
                    .iter()
                    .map(|(l, t)| {
                        let mut v = to_value(t);
                        v.as_object_mut().unwrap().insert("list".into(), json!(l));
                        v
                    })
                    .collect();
                out.json_value(&json!(arr));
            } else {
                let _ = &board;
                for (l, t) in &rows {
                    println!(
                        "{}  {}  {}",
                        id_style(&t.id),
                        t.title,
                        dim(&format!("({l})"))
                    );
                }
            }
        }
    }
    Ok(())
}

/// `wipe comment ...`
pub fn comment(out: &Out, cmd: CommentCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        CommentCmd::Add {
            ticket,
            body,
            author,
        } => {
            let who = identity::resolve(author);
            let cid = ops::add_comment(&s, &ticket, &who, &body, Utc::now())?;
            out.ok(
                format!("commented on {ticket} ({cid})"),
                json!({ "ok": true, "ticket": ticket, "comment": cid, "author": who }),
            );
        }
        CommentCmd::List { ticket } => {
            let t = s.load_ticket(&ticket)?;
            if out.json {
                out.json_value(&json!({ "ticket": ticket, "comments": t.comments.iter().map(to_value).collect::<Vec<_>>() }));
            } else if t.comments.is_empty() {
                out.line(format!("{ticket} has no comments"));
            } else {
                for c in &t.comments {
                    println!("{} {}\n  {}", id_style(&c.id), dim(&c.author), c.body);
                }
            }
        }
        CommentCmd::Remove { ticket, comment } => {
            ops::delete_comment(&s, &ticket, &comment, Utc::now())?;
            out.ok(
                format!("removed {comment} from {ticket}"),
                json!({ "ok": true, "ticket": ticket, "comment": comment }),
            );
        }
    }
    Ok(())
}

/// `wipe checklist ...` - manage a ticket's checklist items.
pub fn checklist(out: &Out, cmd: ChecklistCmd) -> Result<()> {
    checks(out, ops::Checks::Checklist, cmd)
}

/// `wipe criteria ...` - manage a ticket's acceptance criteria.
pub fn criteria(out: &Out, cmd: ChecklistCmd) -> Result<()> {
    checks(out, ops::Checks::Acceptance, cmd)
}

/// Shared implementation for the two tickable surfaces (checklist / criteria).
fn checks(out: &Out, kind: ops::Checks, cmd: ChecklistCmd) -> Result<()> {
    let s = store()?;
    // The human/JSON wording for this surface: (spoken name, JSON key).
    let (noun, key) = match kind {
        ops::Checks::Checklist => ("checklist", "checklist"),
        ops::Checks::Acceptance => ("acceptance criteria", "acceptance"),
    };
    match cmd {
        ChecklistCmd::Add { ticket, text } => {
            let id = ops::checks_add(&s, kind, &ticket, &text, Utc::now())?;
            out.ok(
                format!("added {id} to {ticket}"),
                json!({ "ok": true, "ticket": ticket, "item": id }),
            );
        }
        ChecklistCmd::List { ticket } => {
            let t = s.load_ticket(&ticket)?;
            let items = match kind {
                ops::Checks::Checklist => &t.checklist,
                ops::Checks::Acceptance => &t.acceptance,
            };
            if out.json {
                out.json_value(&json!({
                    "ticket": ticket,
                    key: items.iter().map(to_value).collect::<Vec<_>>(),
                }));
            } else if items.is_empty() {
                out.line(format!("{ticket} has no {noun}"));
            } else {
                let done = items.iter().filter(|i| i.done).count();
                println!("{ticket} {noun} ({done}/{})", items.len());
                for i in items {
                    let box_ = if i.done { "[x]" } else { "[ ]" };
                    println!("  {box_} {} {}", id_style(&i.id), i.text);
                }
            }
        }
        ChecklistCmd::Check { ticket, item } => {
            ops::checks_set(&s, kind, &ticket, &item, Some(true), Utc::now())?;
            out.ok(
                format!("checked {item}"),
                json!({ "ok": true, "ticket": ticket, "item": item, "done": true }),
            );
        }
        ChecklistCmd::Uncheck { ticket, item } => {
            ops::checks_set(&s, kind, &ticket, &item, Some(false), Utc::now())?;
            out.ok(
                format!("unchecked {item}"),
                json!({ "ok": true, "ticket": ticket, "item": item, "done": false }),
            );
        }
        ChecklistCmd::Toggle { ticket, item } => {
            let done = ops::checks_set(&s, kind, &ticket, &item, None, Utc::now())?;
            out.ok(
                format!("{} {item}", if done { "checked" } else { "unchecked" }),
                json!({ "ok": true, "ticket": ticket, "item": item, "done": done }),
            );
        }
        ChecklistCmd::Edit { ticket, item, text } => {
            ops::checks_edit(&s, kind, &ticket, &item, &text, Utc::now())?;
            out.ok(
                format!("edited {item}"),
                json!({ "ok": true, "ticket": ticket, "item": item }),
            );
        }
        ChecklistCmd::Remove { ticket, item } => {
            ops::checks_remove(&s, kind, &ticket, &item, Utc::now())?;
            out.ok(
                format!("removed {item} from {ticket}"),
                json!({ "ok": true, "ticket": ticket, "item": item }),
            );
        }
        ChecklistCmd::Move {
            ticket,
            item,
            index,
        } => {
            ops::checks_move(&s, kind, &ticket, &item, index, Utc::now())?;
            out.ok(
                format!("moved {item} to position {index}"),
                json!({ "ok": true, "ticket": ticket, "item": item, "index": index }),
            );
        }
    }
    Ok(())
}

/// `wipe label ...`
pub fn label(out: &Out, cmd: LabelCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        LabelCmd::Create {
            name,
            color,
            description,
        } => {
            let label = ops::create_label(&s, &name, color, description)?;
            out.ok(
                format!(
                    "created label '{}' ({})",
                    label.name,
                    label.color.clone().unwrap_or_default()
                ),
                to_value(&label),
            );
        }
        LabelCmd::List => {
            let defs = s.load_definitions()?;
            if out.json {
                out.json_value(
                    &json!({ "labels": defs.labels.iter().map(to_value).collect::<Vec<_>>() }),
                );
            } else {
                for l in &defs.labels {
                    let color = l.color.clone().unwrap_or_default();
                    println!("{}  {}", l.name, dim(&color));
                }
            }
        }
        LabelCmd::Delete { name } => {
            ops::delete_label(&s, &name, Utc::now())?;
            out.ok(
                format!("deleted label '{name}'"),
                json!({ "ok": true, "name": name }),
            );
        }
        LabelCmd::Assign { ticket, name } => {
            let t = s.load_ticket(&ticket)?;
            let mut labels = t.labels.clone();
            if !labels.contains(&name) {
                labels.push(name.clone());
            }
            let patch = TicketPatch {
                labels: Some(labels),
                ..Default::default()
            };
            let t = ops::update_ticket(&s, &ticket, patch, &identity::resolve(None), Utc::now())?;
            out.ok(format!("labeled {ticket} '{name}'"), to_value(&t));
        }
        LabelCmd::Remove { ticket, name } => {
            let t = s.load_ticket(&ticket)?;
            let labels: Vec<String> = t.labels.iter().filter(|l| *l != &name).cloned().collect();
            let patch = TicketPatch {
                labels: Some(labels),
                ..Default::default()
            };
            let t = ops::update_ticket(&s, &ticket, patch, &identity::resolve(None), Utc::now())?;
            out.ok(
                format!("removed label '{name}' from {ticket}"),
                to_value(&t),
            );
        }
    }
    Ok(())
}

/// `wipe media ...`
pub fn media(out: &Out, cmd: MediaCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        MediaCmd::Add { ticket, path } => {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow!("invalid file name: {}", path.display()))?
                .to_string();
            let bytes =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let limit = s.load_settings()?.max_attachment_mb * 1024 * 1024;
            if bytes.len() as u64 > limit {
                bail!(
                    "{} is {:.1} MB, over the {} MB attachment limit",
                    name,
                    bytes.len() as f64 / 1_048_576.0,
                    limit / 1024 / 1024
                );
            }
            let att = ops::add_attachment(
                &s,
                &ticket,
                &name,
                &bytes,
                guess_mime(&name),
                &identity::resolve(None),
                Utc::now(),
            )?;
            let where_ = match att.source {
                wipe_core::model::AttachmentSource::Repo => "referenced from repo",
                wipe_core::model::AttachmentSource::Media => "stored in .wipe/media",
            };
            out.ok(
                format!("attached {} to {ticket} ({where_})", att.name),
                to_value(&att),
            );
        }
        MediaCmd::List { ticket } => {
            let t = s.load_ticket(&ticket)?;
            if out.json {
                out.json_value(&json!({ "ticket": ticket, "attachments": t.attachments }));
            } else {
                for a in &t.attachments {
                    println!("{}  {}  {}", id_style(&a.name), dim(&a.path), dim(&a.mime));
                }
            }
        }
        MediaCmd::Remove { ticket, name } => {
            let t = s.load_ticket(&ticket)?;
            let path = t
                .attachments
                .iter()
                .find(|a| a.name == name || a.path == name)
                .map(|a| a.path.clone())
                .ok_or_else(|| anyhow!("no attachment `{name}` on {ticket}"))?;
            ops::remove_attachment(&s, &ticket, &path, &identity::resolve(None), Utc::now())?;
            out.ok(
                format!("detached {name} from {ticket}"),
                json!({ "ok": true, "ticket": ticket }),
            );
        }
    }
    Ok(())
}

/// Best-effort MIME type from a file extension.
pub(crate) fn guess_mime(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        "md" => "text/markdown",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// `wipe config ...` - project settings, or user defaults with `--global`.
pub fn config(out: &Out, global: bool, cmd: ConfigCmd) -> Result<()> {
    if global {
        return config_global(out, cmd);
    }
    let s = store()?;
    match cmd {
        ConfigCmd::Show => {
            let settings = s.load_settings()?;
            let board = s.load_board()?;
            if out.json {
                let mut v = to_value(&settings);
                v.as_object_mut()
                    .unwrap()
                    .insert("board.name".into(), json!(board.name));
                out.json_value(&v);
            } else {
                println!("board.name          {}", board.name);
                println!("daemon.port         {}", settings.daemon.port);
                println!(
                    "daemon.expose       {}",
                    expose_slug(settings.daemon.expose)
                );
                println!("daemon.autoserve    {}", settings.daemon.autoserve);
                println!("daemon.idle_timeout {}", settings.daemon.idle_timeout_secs);
            }
        }
        ConfigCmd::Get { key } => {
            let settings = s.load_settings()?;
            let value = match key.as_str() {
                "daemon.port" => json!(settings.daemon.port),
                "daemon.expose" => json!(expose_slug(settings.daemon.expose)),
                "daemon.autoserve" => json!(settings.daemon.autoserve),
                "daemon.idle_timeout" => json!(settings.daemon.idle_timeout_secs),
                "board.name" => json!(s.load_board()?.name),
                other => bail!("unknown config key '{other}'"),
            };
            if out.json {
                out.json_value(&json!({ "key": key, "value": value }));
            } else {
                println!("{value}");
            }
        }
        ConfigCmd::Set { key, value } => {
            match key.as_str() {
                "daemon.port" => {
                    let mut settings = s.load_settings()?;
                    settings.daemon.port =
                        value.parse().context("port must be a number 0-65535")?;
                    s.save_settings(&settings)?;
                }
                "daemon.expose" => {
                    let mut settings = s.load_settings()?;
                    settings.daemon.expose = match value.as_str() {
                        "none" => Exposure::None,
                        "tailscale" => Exposure::Tailscale,
                        "proxy" => Exposure::Proxy,
                        other => bail!("expose must be none|tailscale|proxy, got '{other}'"),
                    };
                    s.save_settings(&settings)?;
                }
                "daemon.autoserve" => {
                    let mut settings = s.load_settings()?;
                    settings.daemon.autoserve = parse_bool(&value)?;
                    s.save_settings(&settings)?;
                }
                "daemon.idle_timeout" => {
                    let mut settings = s.load_settings()?;
                    settings.daemon.idle_timeout_secs = value
                        .parse()
                        .context("idle_timeout must be seconds (a number)")?;
                    s.save_settings(&settings)?;
                }
                "board.name" => {
                    let mut b = s.load_board()?;
                    b.name = value.clone();
                    b.updated = Utc::now();
                    s.save_board(&b)?;
                }
                other => bail!("unknown config key '{other}'"),
            }
            out.ok(
                format!("set {key} = {value}"),
                json!({ "ok": true, "key": key, "value": value }),
            );
        }
    }
    Ok(())
}

/// `wipe config --global ...` - the machine-wide user defaults.
fn config_global(out: &Out, cmd: ConfigCmd) -> Result<()> {
    match cmd {
        ConfigCmd::Show => {
            let g = GlobalConfig::load();
            if out.json {
                out.json_value(&to_value(&g));
            } else {
                let path = GlobalConfig::path()
                    .map(|p| clean_path(&p))
                    .unwrap_or_else(|| "(unavailable)".into());
                println!("{}", dim(&format!("# {path}")));
                println!("default.port   {}", opt(g.default_port));
                println!(
                    "default.expose {}",
                    g.default_expose.map(expose_slug).unwrap_or("-")
                );
                println!("autoserve      {}", opt(g.autoserve));
                println!("idle           {}", opt(g.idle_timeout_secs));
                println!(
                    "autostart      {} {}",
                    opt(g.autostart),
                    dim(&format!(
                        "(login entry {})",
                        if autostart::is_enabled() {
                            "present"
                        } else {
                            "absent"
                        }
                    ))
                );
                println!(
                    "starter        {}",
                    g.starter.map(starter_slug).unwrap_or("-")
                );
                println!(
                    "skill.target   {}",
                    g.skill_target.as_deref().unwrap_or("-")
                );
                println!("skill.global   {}", opt(g.skill_global));
                println!("ui.accent      {}", g.ui_accent.as_deref().unwrap_or("-"));
                println!("ui.theme       {}", g.ui_theme.as_deref().unwrap_or("-"));
                println!(
                    "identity.default {}",
                    g.default_identity.as_deref().unwrap_or("-")
                );
                println!("identity.prefer  {}", opt(g.prefer_default_identity));
                println!(
                    "scan.roots       {}",
                    g.scan_roots
                        .as_ref()
                        .filter(|r| !r.is_empty())
                        .map(|r| r.join(", "))
                        .unwrap_or_else(|| "(home)".into())
                );
            }
        }
        ConfigCmd::Get { key } => {
            let g = GlobalConfig::load();
            let value = match key.as_str() {
                "default.port" => json!(g.default_port),
                "default.expose" => json!(g.default_expose.map(expose_slug)),
                "autoserve" => json!(g.autoserve),
                "idle" => json!(g.idle_timeout_secs),
                "autostart" => json!(g.autostart),
                "starter" => json!(g.starter.map(starter_slug)),
                "skill.target" => json!(g.skill_target),
                "skill.global" => json!(g.skill_global),
                "ui.accent" => json!(g.ui_accent),
                "ui.theme" => json!(g.ui_theme),
                "identity.default" => json!(g.default_identity),
                "identity.prefer" => json!(g.prefer_default_identity),
                "scan.roots" => json!(g.scan_roots),
                other => bail!("unknown global key '{other}'"),
            };
            if out.json {
                out.json_value(&json!({ "key": key, "value": value }));
            } else {
                println!("{value}");
            }
        }
        ConfigCmd::Set { key, value } => {
            let mut g = GlobalConfig::load();
            match key.as_str() {
                "default.port" => {
                    g.default_port = Some(value.parse().context("port must be 0-65535")?)
                }
                "default.expose" => g.default_expose = Some(parse_expose(&value)?),
                "autoserve" => g.autoserve = Some(parse_bool(&value)?),
                "idle" => {
                    g.idle_timeout_secs = Some(value.parse().context("idle must be seconds")?)
                }
                "autostart" => {
                    let on = parse_bool(&value)?;
                    g.autostart = Some(on);
                    // Reflect the choice in the OS login entry immediately.
                    let r = if on {
                        autostart::enable()
                    } else {
                        autostart::disable()
                    };
                    match r {
                        Ok(note) if !out.json => out.line(format!("  {note}")),
                        Ok(_) => {}
                        Err(e) => out.line(format!("  (autostart change failed: {e})")),
                    }
                }
                "starter" => g.starter = Some(onboard::parse_starter(&value)?),
                "skill.target" => {
                    if !matches!(value.as_str(), "claude" | "agents") {
                        bail!("skill.target must be claude|agents");
                    }
                    g.skill_target = Some(value.clone());
                }
                "skill.global" => g.skill_global = Some(parse_bool(&value)?),
                "ui.accent" => {
                    if !matches!(value.as_str(), "book-cloth" | "kraft" | "focus" | "sage") {
                        bail!("ui.accent must be book-cloth|kraft|focus|sage");
                    }
                    g.ui_accent = Some(value.clone());
                }
                "ui.theme" => {
                    if !matches!(value.as_str(), "light" | "dark" | "system") {
                        bail!("ui.theme must be light|dark|system");
                    }
                    g.ui_theme = Some(value.clone());
                }
                "identity.default" => {
                    if value.trim().is_empty() {
                        bail!("identity.default cannot be empty (use e.g. 'human')");
                    }
                    g.default_identity = Some(value.clone());
                }
                "identity.prefer" => g.prefer_default_identity = Some(parse_bool(&value)?),
                "scan.roots" => {
                    // Comma- or semicolon-separated list of directories.
                    let roots: Vec<String> = value
                        .split([',', ';'])
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    g.scan_roots = if roots.is_empty() { None } else { Some(roots) };
                }
                other => bail!("unknown global key '{other}'"),
            }
            g.save().context("saving global config")?;
            out.ok(
                format!("set (global) {key} = {value}"),
                json!({ "ok": true, "key": key, "value": value }),
            );
        }
    }
    Ok(())
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "-".into())
}

fn expose_slug(e: Exposure) -> &'static str {
    match e {
        Exposure::None => "none",
        Exposure::Tailscale => "tailscale",
        Exposure::Proxy => "proxy",
    }
}

fn parse_expose(s: &str) -> Result<Exposure> {
    Ok(match s {
        "none" => Exposure::None,
        "tailscale" => Exposure::Tailscale,
        "proxy" => Exposure::Proxy,
        other => bail!("expose must be none|tailscale|proxy, got '{other}'"),
    })
}

fn parse_bool(s: &str) -> Result<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        other => bail!("expected true/false, got '{other}'"),
    }
}

/// `wipe doctor`
pub fn doctor(out: &Out) -> Result<()> {
    let in_board = Store::discover(".").ok();
    let git = identity::git_available();
    let author = identity::resolve(None);
    let author_source = identity::source(None);
    let detected = vcs::detect(std::path::Path::new("."));
    let (board_name, tickets) = match &in_board {
        Some(s) => (Some(s.load_board()?.name), s.ticket_ids()?.len()),
        None => (None, 0),
    };
    if out.json {
        out.json_value(&json!({
            "in_board": in_board.is_some(),
            "board": board_name,
            "tickets": tickets,
            "git_available": git,
            "vcs": detected.name(),
            "author": author,
            "author_source": author_source,
            "version": env!("CARGO_PKG_VERSION"),
        }));
    } else {
        let mark = |b: bool| if b { "✓" } else { "✗" };
        println!("wipe {}", env!("CARGO_PKG_VERSION"));
        println!(
            "{} inside a board{}",
            mark(in_board.is_some()),
            board_name
                .map(|n| format!(": {n} ({tickets} tickets)"))
                .unwrap_or_default()
        );
        println!("{} git available", mark(git));
        println!("  vcs: {}", detected.name());
        println!(
            "  identity: {author}  {}",
            dim(&format!("({author_source})"))
        );
    }
    Ok(())
}

/// `wipe skill [show|install|path]`
pub fn skill(out: &Out, cmd: Option<SkillCmd>) -> Result<()> {
    match cmd.unwrap_or(SkillCmd::Show) {
        SkillCmd::Show => {
            if out.json {
                out.json_value(&json!({ "skill": SKILL }));
            } else {
                print!("{SKILL}");
            }
        }
        SkillCmd::Install(a) => {
            let force = a.force;
            let p = skills::plan(&a)?;
            skills::install(&p, SKILL, force)?;
            let path = clean_path(&p.file);
            out.ok(
                format!("installed wipe skill for {} at {path}", p.target.label()),
                json!({ "ok": true, "target": p.target.slug(), "global": p.global, "path": path }),
            );
            if !out.json {
                println!(
                    "  agents that read {} skills pick it up automatically.",
                    p.target.slug()
                );
            }
        }
        SkillCmd::Path(a) => {
            let p = skills::plan(&a)?;
            let path = clean_path(&p.file);
            if out.json {
                out.json_value(
                    &json!({ "target": p.target.slug(), "global": p.global, "path": path }),
                );
            } else {
                println!("{path}");
            }
        }
    }
    Ok(())
}

/// `wipe serve` - start the local daemon serving the board UI + API.
///
/// `serve` is a global human convenience, not bound to one board: run inside a
/// project and it opens that board by default; run anywhere else and it starts a
/// viewer over every board you have opened before. Either way the UI can switch
/// between projects and every edit targets whichever board is on screen.
pub fn serve(out: &Out, args: ServeArgs) -> Result<()> {
    let g = GlobalConfig::load();
    // A board here is optional. When present it supplies the default project and
    // its saved daemon settings; when absent we fall back to the global defaults.
    let board = Store::discover(".").ok();
    let settings = match &board {
        Some(s) => s.load_settings()?,
        None => {
            let mut d = wipe_core::model::Settings::default();
            if let Some(p) = g.default_port {
                d.daemon.port = p;
            }
            d.daemon.autoserve = g.autoserve.unwrap_or(d.daemon.autoserve);
            d.daemon.idle_timeout_secs = g.idle_timeout_secs.unwrap_or(d.daemon.idle_timeout_secs);
            d
        }
    };
    let port = args.port.unwrap_or(settings.daemon.port);

    // If a wipe daemon is already serving this port, don't fail with a bind error;
    // point the user at it instead.
    if let Some(url) = detect_running(port) {
        out.ok(
            format!("wipe is already serving at {url} - open that, or stop it to serve here"),
            json!({ "ok": true, "already_running": true, "url": url }),
        );
        return Ok(());
    }

    // Discover every board on disk so the UI lists them all - crucial when serving
    // globally (no board here), where the registry alone might be empty on a fresh
    // machine. Also include the current directory as a scan root.
    registry::prune();
    if board.is_none() {
        out.line("scanning for boards…");
        let mut roots = configured_scan_roots();
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd);
        }
        let found = registry::scan(&roots, 7);
        if !found.is_empty() {
            out.line(format!("  found {} board(s)", found.len()));
        }
    }

    // Idle-shutdown: --idle overrides (0 = never); otherwise honor autoserve.
    let idle = match args.idle {
        Some(0) => None,
        Some(secs) => Some(std::time::Duration::from_secs(secs)),
        None => settings
            .daemon
            .autoserve
            .then(|| std::time::Duration::from_secs(settings.daemon.idle_timeout_secs)),
    };

    let cfg = wipe_daemon::ServeConfig {
        root: board.as_ref().map(|s| s.root().to_path_buf()),
        port,
        expose: settings.daemon.expose,
        open: args.open,
        idle_timeout: idle,
    };
    match &board {
        Some(s) => out.line(format!(
            "starting wipe UI for '{}' on port {port}…",
            s.load_board()?.name
        )),
        None => out.line(format!(
            "starting wipe UI on port {port}… (no board here; pick one in the UI)"
        )),
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting async runtime")?;
    rt.block_on(wipe_daemon::serve(cfg))?;
    Ok(())
}

/// Probe `127.0.0.1:port` for an already-running wipe daemon. Returns its URL if
/// `/api/health` responds and identifies as `wipe-daemon`; `None` otherwise
/// (nothing listening, or some other service holds the port).
fn detect_running(port: u16) -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(600)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(600)))
        .ok()?;
    let req =
        format!("GET /api/health HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    buf.contains("wipe-daemon")
        .then(|| format!("http://127.0.0.1:{port}"))
}

// ---------------------------------------------------------------------------

fn done_list(board: &wipe_core::model::Board) -> Option<String> {
    board
        .lists
        .iter()
        .find(|l| l.id == "done")
        .map(|l| l.id.clone())
        .or_else(|| board.lists.last().map(|l| l.id.clone()))
}

fn print_ticket_human(t: &wipe_core::model::Ticket, list_id: Option<&str>) {
    println!("{}  {}", id_style(&t.id), t.title);
    if let Some(l) = list_id {
        println!("  {}", dim(&format!("list: {l}")));
    }
    if let Some(p) = &t.priority {
        println!("  {}", dim(&format!("priority: {p}")));
    }
    if !t.labels.is_empty() {
        println!("  {}", dim(&format!("labels: {}", t.labels.join(", "))));
    }
    if !t.assignees.is_empty() {
        println!(
            "  {}",
            dim(&format!("assignees: {}", t.assignees.join(", ")))
        );
    }
    if !t.body.is_empty() {
        println!("\n{}", t.body);
    }
    if !t.comments.is_empty() {
        println!("\n{}", dim(&format!("{} comment(s):", t.comments.len())));
        for c in &t.comments {
            println!("  {} {}: {}", id_style(&c.id), dim(&c.author), c.body);
        }
    }
}
