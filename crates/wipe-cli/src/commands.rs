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
use crate::input;
use crate::onboard;
use crate::output::{dim, id_style, Out};
use crate::skills;
use crate::view;

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
        println!("    wipe identity use <you> --human  choose who your CLI writes are by");
        println!("    wipe serve                      open the board UI");
        println!("    wipe ticket create \"...\" --list todo   add your first card");
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
            let active = crate::identity::resolve_opt(None);
            if out.json {
                out.json_value(&json!({
                    "active": active,
                    "source": crate::identity::source(None),
                    "session_detected": crate::identity::session_key().is_some(),
                    "identities": ids.iter().map(to_value).collect::<Vec<_>>(),
                }));
            } else {
                if ids.is_empty() {
                    out.line("no identities yet - `wipe identity use <id>` to create one");
                }
                if active.is_none() {
                    out.line(dim(
                        "no identity chosen in this session - `wipe identity use <id>` to pick one",
                    ));
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
                println!("  scripts or tools that run wipe outside this session can pin it with:");
                println!("    {hint}");
            }
        }
        IdentityCmd::Whoami => {
            let who = crate::identity::resolve_opt(None);
            let src = crate::identity::source(None);
            let warning = crate::identity::divergence();
            if let Some(w) = &warning {
                if !out.json {
                    out.line(format!("  {}", dim(&format!("warning: {w}"))));
                }
            }
            let human = match &who {
                Some(w) => format!("{w}  ({src})"),
                None => "no identity chosen - writes are refused until you run \
                         `wipe identity use <id>` (or set $WIPE_AGENT / pass --agentid)"
                    .to_string(),
            };
            out.ok(
                human,
                json!({
                    "identity": who,
                    "source": src,
                    "session_detected": crate::identity::session_key().is_some(),
                    "warning": warning,
                }),
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

/// Unread inbox events for `who` (since their read cursor), without advancing it.
fn unread_count(s: &Store, who: &str) -> usize {
    let since =
        wipe_core::inbox::read_cursor(s, who).unwrap_or(chrono::DateTime::<Utc>::UNIX_EPOCH);
    wipe_core::inbox::inbox(s, who, since)
        .map(|e| e.len())
        .unwrap_or(0)
}

/// `(board, lists-with-tickets, done list id)` plus the workflow list ids.
fn done_list_id(s: &Store, board: &wipe_core::model::Board) -> String {
    let settings = s.load_settings().unwrap_or_default();
    ops::workflow_lists(board, &settings).1.unwrap_or_default()
}

/// `wipe status`
pub fn status(out: &Out, a: StatusArgs) -> Result<()> {
    let s = store()?;
    let (board, view) = ops::board_view(&s)?;
    for x in &a.exclude_lists {
        ensure_list(&board, x)?;
    }
    let view: Vec<_> = view
        .into_iter()
        .filter(|(l, _)| !a.exclude_lists.contains(l))
        .collect();
    let done = done_list_id(&s, &board);
    let who = identity::resolve_opt(None);
    let unread = who.as_deref().map(|w| unread_count(&s, w));

    if out.json {
        if a.full {
            let lists: Vec<Value> = view
                .iter()
                .map(|(list_id, tickets)| {
                    json!({ "list": list_id, "tickets": tickets.iter().map(to_value).collect::<Vec<_>>() })
                })
                .collect();
            out.json_value(&json!({ "board": board.name, "lists": lists }));
            return Ok(());
        }
        // Rows sit inside their list's object, so they don't repeat the list id.
        let fields: Vec<String> = view::fields(&[])?
            .into_iter()
            .filter(|f| f != "list")
            .collect();
        let lists: Vec<Value> = view
            .iter()
            .map(|(list_id, tickets)| {
                let name = board.list(list_id).map(|l| l.name.as_str()).unwrap_or(list_id);
                if *list_id == done && !a.all {
                    return json!({ "id": list_id, "name": name, "count": tickets.len(), "collapsed": true });
                }
                let rows: Vec<Value> = tickets
                    .iter()
                    .map(|t| view::row(t, list_id, &board, &done, &fields))
                    .collect();
                json!({ "id": list_id, "name": name, "count": tickets.len(), "tickets": rows })
            })
            .collect();
        out.json_value(&json!({
            "board": board.name,
            "identity": who,
            "unread": unread,
            "lists": lists,
        }));
        return Ok(());
    }
    println!(
        "{}  {}",
        board.name.as_str(),
        dim(&format!("({} lists)", board.lists.len()))
    );
    for (list, tickets) in &view {
        let name = board.list(list).map(|l| l.name.as_str()).unwrap_or(list);
        if *list == done && !a.all && !tickets.is_empty() {
            println!(
                "\n{} {}  {}",
                name,
                dim(&format!("[{}]", tickets.len())),
                dim("(collapsed - `wipe status --all` lists them)")
            );
            continue;
        }
        println!("\n{} {}", name, dim(&format!("[{}]", tickets.len())));
        for t in tickets {
            println!(
                "  {}  {}{}",
                id_style(&t.id),
                t.title,
                row_tags(t, &board, &done)
            );
        }
    }
    println!();
    match (&who, unread) {
        (Some(w), Some(n)) if n > 0 => println!(
            "{}",
            dim(&format!("{n} unread for {w} - `wipe inbox --unread`"))
        ),
        (Some(_), _) => {}
        (None, _) => println!(
            "{}",
            dim("no identity chosen - `wipe identity use <id>` before writing")
        ),
    }
    println!(
        "{}",
        dim(
            "one list, compact: `wipe ticket list --list <id>` · a ticket: `wipe ticket show <id>`"
        )
    );
    Ok(())
}

/// The dim `[labels] @assignees ⏳T-3` suffix on a human ticket line.
fn row_tags(t: &wipe_core::model::Ticket, board: &wipe_core::model::Board, done: &str) -> String {
    let mut tags = Vec::new();
    if !t.labels.is_empty() {
        tags.push(format!("[{}]", t.labels.join(",")));
    }
    for a in &t.assignees {
        tags.push(format!("@{a}"));
    }
    let open = ops::open_blockers(t, board, done);
    if !open.is_empty() {
        tags.push(format!("blocked by {}", open.join(",")));
    }
    if tags.is_empty() {
        String::new()
    } else {
        format!("  {}", dim(&tags.join(" ")))
    }
}

/// Fail with the available list ids when `id` is not a list on the board.
fn ensure_list(board: &wipe_core::model::Board, id: &str) -> Result<()> {
    if board.list(id).is_some() {
        return Ok(());
    }
    let avail: Vec<&str> = board.lists.iter().map(|l| l.id.as_str()).collect();
    bail!(
        "no list `{id}` - available lists: {}",
        if avail.is_empty() {
            "(none yet - add one with `wipe list add`)".to_string()
        } else {
            avail.join(", ")
        }
    )
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

/// The id of the list currently holding `id`, if any.
fn list_of(s: &Store, id: &str) -> Option<String> {
    s.load_board()
        .ok()
        .and_then(|b| b.locate_card(id).map(|(l, _)| l))
}

/// Short write receipt for a ticket: `{"ok":true,"id":..,"list":..}` plus extras.
fn ticket_receipt(s: &Store, id: &str, extra: Value) -> Value {
    let mut v = json!({ "ok": true, "id": id, "list": list_of(s, id) });
    if let (Some(m), Value::Object(e)) = (v.as_object_mut(), extra) {
        m.extend(e);
    }
    v
}

/// Full ticket JSON (with its list) for `--echo`.
fn ticket_full(s: &Store, id: &str) -> Value {
    match s.load_ticket(id) {
        Ok(t) => {
            let mut v = to_value(&t);
            if let Some(m) = v.as_object_mut() {
                m.insert("list".into(), json!(list_of(s, id)));
            }
            v
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

/// Workflow lists for this board: `(review, done, rework)`.
fn workflow(s: &Store) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let board = s.load_board()?;
    let settings = s.load_settings()?;
    Ok(ops::workflow_lists(&board, &settings))
}

/// Criteria on `id` that are not ticked yet, as `ac-n: text`.
fn open_criteria(s: &Store, id: &str) -> Result<Vec<String>> {
    Ok(s.load_ticket(id)?
        .acceptance
        .iter()
        .filter(|c| !c.done)
        .map(|c| format!("{}: {}", c.id, c.text))
        .collect())
}

/// `wipe ticket ...`
pub fn ticket(out: &Out, cmd: TicketCmd) -> Result<()> {
    let s = store()?;
    match cmd {
        TicketCmd::Create(a) => {
            let title = match (a.title_pos, a.title) {
                (Some(t), None) | (None, Some(t)) if !t.trim().is_empty() => t,
                _ => bail!(
                    "a title is required - `wipe ticket create \"Add login\" --list <LIST>` \
                     (or --title \"...\")"
                ),
            };
            // A target list is REQUIRED - creating into an implicit "first list" is a
            // footgun (e.g. a leftmost PM-only column). Guide the caller to the real
            // lists rather than silently guessing.
            let list = match a.list {
                Some(l) if !l.trim().is_empty() => l,
                _ => {
                    let board = s.load_board()?;
                    let avail: Vec<&str> = board.lists.iter().map(|l| l.id.as_str()).collect();
                    let example = avail.first().copied().unwrap_or("todo");
                    bail!(
                        "a target list is required - pass --list <LIST>.\n  \
                         available lists: {}\n  \
                         example: wipe ticket create \"Add login\" --list {}",
                        if avail.is_empty() {
                            "(none yet - add one with `wipe list add`)".to_string()
                        } else {
                            avail.join(", ")
                        },
                        example
                    );
                }
            };
            for b in &a.blocked_by {
                s.load_ticket(b)?;
            }
            let body = input::text(a.body, a.body_file.as_deref(), "the body")?;
            let actor = identity::resolve(None)?;
            let now = Utc::now();
            let spec = NewTicket {
                title,
                body,
                priority: a.priority,
                list: Some(list),
                labels: a.labels,
                assignees: a.assignees.clone(),
            };
            let t = ops::create_ticket(&s, spec, &actor, now)?;
            for b in &a.blocked_by {
                ops::add_blocker(&s, &t.id, b, &actor, now)?;
            }
            for who in &a.assignees {
                let _ = wipe_core::inbox::subscribe(&s, who, &t.id);
            }
            out.write(
                format!("created {} - {}", t.id, t.title),
                ticket_receipt(&s, &t.id, json!({ "title": t.title })),
                || ticket_full(&s, &t.id),
            );
        }
        TicketCmd::Show(a) => {
            let mut t = s.load_ticket(&a.id)?;
            let comments_total = t.comments.len();
            if let Some(n) = a.comments {
                t.comments.drain(..comments_total.saturating_sub(n));
            }
            let list_id = list_of(&s, &a.id);
            let commits = if a.no_commits || a.comments_only || !wipe_core::git::is_repo(s.root()) {
                Vec::new()
            } else {
                wipe_core::git::commits_mentioning(s.root(), &t.id, 10).unwrap_or_default()
            };
            if out.json {
                if a.comments_only {
                    out.json_value(&json!({
                        "id": t.id,
                        "title": t.title,
                        "comments": t.comments.iter().map(to_value).collect::<Vec<_>>(),
                        "comments_total": comments_total,
                    }));
                    return Ok(());
                }
                let mut v = to_value(&t);
                let m = v.as_object_mut().expect("ticket is an object");
                m.insert("list".into(), json!(list_id));
                if a.no_activity {
                    m.remove("activity");
                }
                if a.comments.is_some() {
                    m.insert("comments_total".into(), json!(comments_total));
                }
                if !commits.is_empty() {
                    m.insert(
                        "commits".into(),
                        json!(commits
                            .iter()
                            .map(|c| json!({
                                "hash": c.short,
                                "subject": c.subject,
                                "author": c.author_name,
                                "date": c.date,
                            }))
                            .collect::<Vec<_>>()),
                    );
                }
                out.json_value(&v);
            } else if a.comments_only {
                print_comments(&t);
            } else {
                print_ticket_human(&t, list_id.as_deref(), !a.no_activity, &commits);
            }
        }
        TicketCmd::Edit(a) => ticket_edit(out, &s, a)?,
        TicketCmd::Move { id, to, pos } => {
            ops::move_ticket(&s, &id, &to, pos, &identity::resolve(None)?, Utc::now())?;
            out.write(
                format!("moved {id} to {to}"),
                json!({ "ok": true, "id": id, "list": to }),
                || ticket_full(&s, &id),
            );
        }
        TicketCmd::Assign { id, who, remove } => {
            let t = s.load_ticket(&id)?;
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
            let t = ops::update_ticket(&s, &id, patch, &identity::resolve(None)?, Utc::now())?;
            // Auto-subscribe a new assignee to their ticket so it lands in their
            // inbox (best-effort; never fail the assignment over it).
            if !remove {
                let _ = wipe_core::inbox::subscribe(&s, &who, &id);
            }
            let verb = if remove { "unassigned" } else { "assigned" };
            out.write(
                format!("{verb} {who} on {id}"),
                ticket_receipt(&s, &id, json!({ "assignees": t.assignees })),
                || ticket_full(&s, &id),
            );
        }
        TicketCmd::Close { id } => {
            let target = workflow(&s)?
                .1
                .ok_or_else(|| anyhow!("board has no lists"))?;
            ops::move_ticket(
                &s,
                &id,
                &target,
                None,
                &identity::resolve(None)?,
                Utc::now(),
            )?;
            out.write(
                format!("closed {id} (moved to {target})"),
                json!({ "ok": true, "id": id, "list": target }),
                || ticket_full(&s, &id),
            );
        }
        TicketCmd::Reopen { id } => {
            let board = s.load_board()?;
            let target = board
                .lists
                .first()
                .map(|l| l.id.clone())
                .ok_or_else(|| anyhow!("board has no lists"))?;
            ops::move_ticket(
                &s,
                &id,
                &target,
                None,
                &identity::resolve(None)?,
                Utc::now(),
            )?;
            out.write(
                format!("reopened {id} (moved to {target})"),
                json!({ "ok": true, "id": id, "list": target }),
                || ticket_full(&s, &id),
            );
        }
        TicketCmd::Submit(a) => {
            let actor = identity::resolve(None)?;
            let (review, _, _) = workflow(&s)?;
            let target = match a.to.or(review) {
                Some(t) => t,
                None => {
                    let board = s.load_board()?;
                    let avail: Vec<&str> = board.lists.iter().map(|l| l.id.as_str()).collect();
                    bail!(
                        "this board has no review list.\n  \
                         add one (`wipe list add Review`), name one with \
                         `wipe config set board.review_list <list>`, or pass --to <list>.\n  \
                         available lists: {}",
                        avail.join(", ")
                    );
                }
            };
            let msg = input::required(
                a.message,
                a.message_file.as_deref(),
                "a summary of the work",
                "--message",
            )?;
            let mut body = format!("**Submitted for review**\n\n{msg}");
            if !a.tested.is_empty() {
                body.push_str("\n\n**Tested**\n");
                for x in &a.tested {
                    body.push_str(&format!("\n- {x}"));
                }
            }
            if !a.untested.is_empty() {
                body.push_str("\n\n**Not tested**\n");
                for x in &a.untested {
                    body.push_str(&format!("\n- {x}"));
                }
            }
            let now = Utc::now();
            s.load_ticket(&a.id)?;
            ensure_list(&s.load_board()?, &target)?;
            let cid = ops::add_comment(&s, &a.id, &actor, &body, now)?;
            ops::move_ticket(&s, &a.id, &target, None, &actor, now)?;
            // The submitter hears back (approval/rejection) through their inbox.
            let _ = wipe_core::inbox::subscribe(&s, &actor, &a.id);
            let open = open_criteria(&s, &a.id)?;
            if !open.is_empty() && !out.json {
                out.line(dim(&format!(
                    "  note: {} acceptance criteria still unchecked: {}",
                    open.len(),
                    open.join("; ")
                )));
            }
            out.write(
                format!("submitted {} for review (moved to {target}, {cid})", a.id),
                json!({ "ok": true, "id": a.id, "list": target, "comment": cid, "criteria_open": open.len() }),
                || ticket_full(&s, &a.id),
            );
        }
        TicketCmd::Approve(a) => {
            let actor = identity::resolve(None)?;
            let target = workflow(&s)?
                .1
                .ok_or_else(|| anyhow!("board has no lists"))?;
            let open = open_criteria(&s, &a.id)?;
            if !open.is_empty() && !a.force {
                bail!(
                    "{} has {} unchecked acceptance criteria:\n    {}\n  \
                     tick them (`wipe criteria check {} <ac-id>`), `wipe ticket reject` it, \
                     or pass --force to approve anyway",
                    a.id,
                    open.len(),
                    open.join("\n    "),
                    a.id
                );
            }
            let msg = input::text(a.message, None, "the comment")?;
            let now = Utc::now();
            let cid = match msg {
                Some(m) => Some(ops::add_comment(
                    &s,
                    &a.id,
                    &actor,
                    &format!("**Approved**\n\n{m}"),
                    now,
                )?),
                None => None,
            };
            ops::move_ticket(&s, &a.id, &target, None, &actor, now)?;
            out.write(
                format!("approved {} (moved to {target})", a.id),
                json!({ "ok": true, "id": a.id, "list": target, "comment": cid }),
                || ticket_full(&s, &a.id),
            );
        }
        TicketCmd::Reject(a) => {
            let actor = identity::resolve(None)?;
            let reason = input::required(
                a.message,
                a.message_file.as_deref(),
                "a reason for sending it back",
                "--message",
            )?;
            let target = match a.to {
                Some(t) => t,
                None => workflow(&s)?
                    .2
                    .ok_or_else(|| anyhow!("board has no lists"))?,
            };
            s.load_ticket(&a.id)?;
            ensure_list(&s.load_board()?, &target)?;
            let now = Utc::now();
            let cid = ops::add_comment(
                &s,
                &a.id,
                &actor,
                &format!("**Changes requested**\n\n{reason}"),
                now,
            )?;
            ops::move_ticket(&s, &a.id, &target, None, &actor, now)?;
            out.write(
                format!("sent {} back to {target} ({cid})", a.id),
                json!({ "ok": true, "id": a.id, "list": target, "comment": cid }),
                || ticket_full(&s, &a.id),
            );
        }
        TicketCmd::Block(a) => {
            let actor = identity::resolve(None)?;
            let now = Utc::now();
            let mut added = Vec::new();
            for b in &a.by {
                if ops::add_blocker(&s, &a.id, b, &actor, now)? {
                    added.push(b.clone());
                }
            }
            let t = s.load_ticket(&a.id)?;
            let all: Vec<&str> = t.blocked_by().collect();
            out.write(
                format!("{} is blocked by {}", a.id, all.join(", ")),
                json!({ "ok": true, "id": a.id, "blocked_by": all, "added": added }),
                || ticket_full(&s, &a.id),
            );
        }
        TicketCmd::Unblock(a) => {
            let actor = identity::resolve(None)?;
            let now = Utc::now();
            let mut removed = Vec::new();
            for b in &a.by {
                if ops::remove_blocker(&s, &a.id, b, &actor, now)? {
                    removed.push(b.clone());
                }
            }
            let t = s.load_ticket(&a.id)?;
            let all: Vec<&str> = t.blocked_by().collect();
            out.write(
                if all.is_empty() {
                    format!("{} is no longer blocked", a.id)
                } else {
                    format!("{} is still blocked by {}", a.id, all.join(", "))
                },
                json!({ "ok": true, "id": a.id, "blocked_by": all, "removed": removed }),
                || ticket_full(&s, &a.id),
            );
        }
        TicketCmd::Delete { id, yes, purge } => {
            if !yes {
                bail!("refusing to delete {id} without --yes");
            }
            if purge {
                ops::delete_ticket(&s, &id, Utc::now())?;
                out.ok(
                    format!("permanently deleted {id}"),
                    json!({ "ok": true, "id": id, "trashed": false }),
                );
            } else {
                let days = GlobalConfig::load().trash_retention_days();
                wipe_core::trash::trash_ticket(&s, &id, days, Utc::now())?;
                out.ok(
                    format!("deleted {id} (restorable from the trash for {days}d)"),
                    json!({ "ok": true, "id": id, "trashed": days > 0, "retention_days": days }),
                );
            }
        }
        TicketCmd::Duplicate { id } => {
            let t = ops::duplicate_ticket(&s, &id, &identity::resolve(None)?, Utc::now())?;
            out.write(
                format!("duplicated {id} -> {}", t.id),
                ticket_receipt(&s, &t.id, json!({ "title": t.title, "from": id })),
                || ticket_full(&s, &t.id),
            );
        }
        TicketCmd::List(a) => ticket_list(out, &s, a)?,
    }
    Ok(())
}

/// `wipe ticket edit` - every field, plus labels/assignees/blockers/list/comment,
/// in one call. Inputs are validated before anything is written.
fn ticket_edit(out: &Out, s: &Store, a: TicketEditArgs) -> Result<()> {
    let actor = identity::resolve(None)?;
    let now = Utc::now();
    let body = input::text(a.body, a.body_file.as_deref(), "the body")?;
    let comment = input::text(a.comment, a.comment_file.as_deref(), "the comment")?;
    let cur = s.load_ticket(&a.id)?;
    if let Some(to) = &a.to {
        ensure_list(&s.load_board()?, to)?;
    }
    for b in a.blocked_by.iter().chain(&a.unblock) {
        s.load_ticket(b)?;
    }

    let mut changed: Vec<&str> = Vec::new();
    // Blockers first: a cycle is the one check that can only fail while writing,
    // and failing here leaves the ticket untouched.
    for b in &a.blocked_by {
        if ops::add_blocker(s, &a.id, b, &actor, now)? {
            changed.push("blocked_by");
        }
    }
    for b in &a.unblock {
        if ops::remove_blocker(s, &a.id, b, &actor, now)? {
            changed.push("blocked_by");
        }
    }
    if let Some(new_author) = &a.author {
        ops::reattribute_ticket(s, &a.id, new_author, &actor, now)?;
        changed.push("author");
    }
    let labels = (!a.labels.is_empty() || !a.remove_labels.is_empty()).then(|| {
        let mut l = cur.labels.clone();
        for x in &a.labels {
            if !l.contains(x) {
                l.push(x.clone());
            }
        }
        l.retain(|x| !a.remove_labels.contains(x));
        l
    });
    let assignees = (!a.assignees.is_empty() || !a.unassign.is_empty()).then(|| {
        let mut l = cur.assignees.clone();
        for x in &a.assignees {
            if !l.contains(x) {
                l.push(x.clone());
            }
        }
        l.retain(|x| !a.unassign.contains(x));
        l
    });
    for (name, set) in [
        ("title", a.title.is_some()),
        ("body", body.is_some()),
        ("priority", a.priority.is_some()),
        ("labels", labels.is_some()),
        ("assignees", assignees.is_some()),
    ] {
        if set {
            changed.push(name);
        }
    }
    if a.title.is_some()
        || body.is_some()
        || a.priority.is_some()
        || labels.is_some()
        || assignees.is_some()
    {
        let patch = TicketPatch {
            title: a.title,
            body,
            // Provided priority sets it; absent leaves it unchanged.
            priority: a.priority.map(Some),
            labels,
            assignees,
        };
        ops::update_ticket(s, &a.id, patch, &actor, now)?;
    }
    for who in &a.assignees {
        let _ = wipe_core::inbox::subscribe(s, who, &a.id);
    }
    if let Some(to) = &a.to {
        ops::move_ticket(s, &a.id, to, None, &actor, now)?;
        changed.push("list");
    }
    let cid = match comment {
        Some(c) => {
            changed.push("comment");
            Some(ops::add_comment(s, &a.id, &actor, &c, now)?)
        }
        None => None,
    };
    if changed.is_empty() {
        bail!(
            "nothing to change - pass e.g. --title, --body/--body-file, --label, --remove-label, \
             --assignee, --unassign, --blocked-by, --unblock, --to <list>, --comment/-m"
        );
    }
    changed.dedup();
    out.write(
        format!("updated {} ({})", a.id, changed.join(", ")),
        ticket_receipt(s, &a.id, json!({ "changed": changed, "comment": cid })),
        || ticket_full(s, &a.id),
    );
    Ok(())
}

/// `wipe ticket list` - compact rows, filtered.
fn ticket_list(out: &Out, s: &Store, a: TicketListArgs) -> Result<()> {
    let now = Utc::now();
    let since = a
        .since
        .as_deref()
        .map(|x| view::parse_since(x, now))
        .transpose()?;
    let fields = view::fields(&a.fields)?;
    let (board, lists) = ops::board_view(s)?;
    for l in a.lists.iter().chain(&a.exclude_lists) {
        ensure_list(&board, l)?;
    }
    let done = done_list_id(s, &board);
    let assignee = match a.assignee.as_deref() {
        Some("me") => Some(identity::resolve(None)?),
        other => other.map(str::to_string),
    };

    let mut rows: Vec<(String, wipe_core::model::Ticket)> = Vec::new();
    for (list_id, tickets) in lists {
        if (!a.lists.is_empty() && !a.lists.contains(&list_id))
            || a.exclude_lists.contains(&list_id)
        {
            continue;
        }
        for t in tickets {
            if !a.labels.iter().all(|l| t.labels.contains(l)) {
                continue;
            }
            if let Some(who) = &assignee {
                if !t.assignees.contains(who) {
                    continue;
                }
            }
            if since.is_some_and(|since| t.updated < since) {
                continue;
            }
            if a.ready || a.blocked {
                let blocked = !ops::open_blockers(&t, &board, &done).is_empty();
                // "Ready" also means "not finished": done work is never next.
                if (a.ready && (blocked || list_id == done)) || (a.blocked && !blocked) {
                    continue;
                }
            }
            rows.push((list_id.clone(), t));
        }
    }
    if let Some(n) = a.limit {
        rows.truncate(n);
    }
    if out.json {
        let arr: Vec<Value> = rows
            .iter()
            .map(|(l, t)| {
                if a.full {
                    let mut v = to_value(t);
                    v.as_object_mut().unwrap().insert("list".into(), json!(l));
                    v
                } else {
                    view::row(t, l, &board, &done, &fields)
                }
            })
            .collect();
        out.json_value(&json!(arr));
    } else if rows.is_empty() {
        out.line("no matching tickets");
    } else {
        for (l, t) in &rows {
            println!(
                "{}  {}  {}{}",
                id_style(&t.id),
                t.title,
                dim(&format!("({l})")),
                row_tags(t, &board, &done)
            );
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
            body_pos,
            body,
            body_file,
            author,
        } => {
            let body = input::required(
                body_pos.or(body),
                body_file.as_deref(),
                "the comment body",
                "--body",
            )?;
            let who = identity::resolve(author)?;
            let cid = ops::add_comment(&s, &ticket, &who, &body, Utc::now())?;
            out.write(
                format!("commented on {ticket} ({cid})"),
                json!({ "ok": true, "ticket": ticket, "comment": cid, "author": who }),
                || ticket_full(&s, &ticket),
            );
        }
        CommentCmd::List { ticket } => {
            let t = s.load_ticket(&ticket)?;
            if out.json {
                out.json_value(&json!({ "ticket": ticket, "comments": t.comments.iter().map(to_value).collect::<Vec<_>>() }));
            } else {
                print_comments(&t);
            }
        }
        CommentCmd::Remove { ticket, comment } => {
            ops::delete_comment(&s, &ticket, &comment, Utc::now())?;
            out.ok(
                format!("removed {comment} from {ticket}"),
                json!({ "ok": true, "ticket": ticket, "comment": comment }),
            );
        }
        CommentCmd::Edit {
            ticket,
            comment,
            body,
            body_file,
        } => {
            let body =
                input::required(body, body_file.as_deref(), "the new comment body", "--body")?;
            ops::edit_comment(&s, &ticket, &comment, &body, Utc::now())?;
            out.ok(
                format!("edited {comment} on {ticket}"),
                json!({ "ok": true, "ticket": ticket, "comment": comment }),
            );
        }
        CommentCmd::Reattribute {
            ticket,
            comment,
            to,
        } => {
            let actor = identity::resolve(None)?;
            let old = ops::reattribute_comment(&s, &ticket, &comment, &to, &actor, Utc::now())?;
            out.ok(
                format!("reattributed {comment} on {ticket}: {old} -> {to}"),
                json!({ "ok": true, "ticket": ticket, "comment": comment, "from": old, "to": to }),
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
        LabelCmd::Assign { ticket, names } => {
            let t = s.load_ticket(&ticket)?;
            let mut labels = t.labels.clone();
            for name in &names {
                if !labels.contains(name) {
                    labels.push(name.clone());
                }
            }
            let patch = TicketPatch {
                labels: Some(labels),
                ..Default::default()
            };
            let t = ops::update_ticket(&s, &ticket, patch, &identity::resolve(None)?, Utc::now())?;
            out.write(
                format!("labeled {ticket} '{}'", names.join("', '")),
                json!({ "ok": true, "id": ticket, "labels": t.labels }),
                || ticket_full(&s, &ticket),
            );
        }
        LabelCmd::Remove { ticket, names } => {
            let t = s.load_ticket(&ticket)?;
            let labels: Vec<String> = t
                .labels
                .iter()
                .filter(|l| !names.contains(l))
                .cloned()
                .collect();
            let patch = TicketPatch {
                labels: Some(labels),
                ..Default::default()
            };
            let t = ops::update_ticket(&s, &ticket, patch, &identity::resolve(None)?, Utc::now())?;
            out.write(
                format!("removed label '{}' from {ticket}", names.join("', '")),
                json!({ "ok": true, "id": ticket, "labels": t.labels }),
                || ticket_full(&s, &ticket),
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
                &identity::resolve(None)?,
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
            ops::remove_attachment(&s, &ticket, &path, &identity::resolve(None)?, Utc::now())?;
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
                println!("daemon.expose       {}", settings.daemon.expose.slug());
                println!("daemon.autoserve    {}", settings.daemon.autoserve);
                println!("daemon.idle_timeout {}", settings.daemon.idle_timeout_secs);
                println!("board.autocommit    {}", settings.autocommit);
                let board = s.load_board()?;
                let (review, done, rework) = ops::workflow_lists(&board, &settings);
                let shown = |set: &Option<String>, eff: Option<String>| match set {
                    Some(v) => v.clone(),
                    None => format!(
                        "{} {}",
                        eff.unwrap_or_else(|| "-".into()),
                        dim("(detected)")
                    ),
                };
                println!(
                    "board.review_list   {}",
                    shown(&settings.review_list, review)
                );
                println!("board.done_list     {}", shown(&settings.done_list, done));
                println!(
                    "board.rework_list   {}",
                    shown(&settings.rework_list, rework)
                );
            }
        }
        ConfigCmd::Get { key } => {
            let settings = s.load_settings()?;
            let value = match key.as_str() {
                "daemon.port" => json!(settings.daemon.port),
                "daemon.expose" => json!(settings.daemon.expose.slug()),
                "daemon.autoserve" => json!(settings.daemon.autoserve),
                "daemon.idle_timeout" => json!(settings.daemon.idle_timeout_secs),
                "board.autocommit" => json!(settings.autocommit),
                "board.review_list" | "board.done_list" | "board.rework_list" => {
                    let board = s.load_board()?;
                    let (review, done, rework) = ops::workflow_lists(&board, &settings);
                    json!(match key.as_str() {
                        "board.review_list" => review,
                        "board.done_list" => done,
                        _ => rework,
                    })
                }
                "board.name" => json!(s.load_board()?.name),
                other => bail!(
                    "unknown config key '{other}' - project keys: {PROJECT_KEYS} (machine-wide: `wipe config --global show`)"
                ),
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
                    settings.daemon.expose = parse_expose(&value)?;
                    s.save_settings(&settings)?;
                }
                "board.review_list" | "board.done_list" | "board.rework_list" => {
                    let mut settings = s.load_settings()?;
                    // An empty value (or `auto`) clears it back to detection.
                    let v = match value.trim() {
                        "" | "auto" => None,
                        id => {
                            ensure_list(&s.load_board()?, id)?;
                            Some(id.to_string())
                        }
                    };
                    match key.as_str() {
                        "board.review_list" => settings.review_list = v,
                        "board.done_list" => settings.done_list = v,
                        _ => settings.rework_list = v,
                    }
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
                "board.autocommit" => {
                    let mut settings = s.load_settings()?;
                    settings.autocommit = parse_bool(&value)?;
                    s.save_settings(&settings)?;
                }
                "board.name" => {
                    let mut b = s.load_board()?;
                    b.name = value.clone();
                    b.updated = Utc::now();
                    s.save_board(&b)?;
                }
                other => bail!(
                    "unknown config key '{other}' - project keys: {PROJECT_KEYS} (machine-wide: `wipe config --global show`)"
                ),
            }
            out.ok(
                format!("set {key} = {value}"),
                json!({ "ok": true, "key": key, "value": value }),
            );
        }
    }
    Ok(())
}

/// The keys `wipe config get/set` understands for a board.
const PROJECT_KEYS: &str = "board.name, board.autocommit, board.review_list, board.done_list, \
board.rework_list, daemon.port, daemon.expose, daemon.autoserve, daemon.idle_timeout";

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
                    g.default_expose.map(Exposure::slug).unwrap_or("-")
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
                    "identity.default {}  {}",
                    g.default_identity.as_deref().unwrap_or("-"),
                    dim("(board UI only - the CLI always asks per session)")
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
                println!("trash.retention_days {}", g.trash_retention_days());
            }
        }
        ConfigCmd::Get { key } => {
            let g = GlobalConfig::load();
            let value = match key.as_str() {
                "default.port" => json!(g.default_port),
                "default.expose" => json!(g.default_expose.map(Exposure::slug)),
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
                "trash.retention_days" => json!(g.trash_retention_days()),
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
                "trash.retention_days" => {
                    g.trash_retention_days = Some(
                        value
                            .parse()
                            .context("retention must be a whole number of days")?,
                    );
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

fn parse_expose(s: &str) -> Result<Exposure> {
    if s.trim().eq_ignore_ascii_case("none") {
        bail!("`none` is ambiguous since 0.4 - use `local` (this machine only) or `lan` (local networks)");
    }
    Exposure::parse(s).ok_or_else(|| anyhow!("expose must be lan|local|tailscale|proxy, got '{s}'"))
}

fn parse_bool(s: &str) -> Result<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        other => bail!("expected true/false, got '{other}'"),
    }
}

/// Turn a user-facing subscribe target into a canonical subscription ref.
/// Tickets (`T-<n>`) and forum posts (`F-...`) are recognized by shape; the
/// keywords `forum`/`all` map to wildcards; anything else is treated as a list id.
fn canonical_sub_ref(target: &str) -> String {
    let t = target.trim();
    match t.to_ascii_lowercase().as_str() {
        "all" | "board" | "*" => return "board:*".to_string(),
        "forum" => return "forum:*".to_string(),
        _ => {}
    }
    if t.starts_with("T-") || t.starts_with("t-") {
        t.to_uppercase()
    } else if t.starts_with("F-") || t.starts_with("f-") {
        format!("forum:{}", t.to_uppercase())
    } else if let Some(rest) = t.strip_prefix("list:") {
        format!("list:{rest}")
    } else if let Some(rest) = t.strip_prefix("forum:") {
        format!("forum:{}", rest.to_uppercase())
    } else {
        format!("list:{t}")
    }
}

/// `wipe subscribe` / `wipe unsubscribe`.
pub fn subscribe(out: &Out, a: SubscribeArgs, remove: bool) -> Result<()> {
    let s = store()?;
    let who = identity::resolve(a.author)?;
    let reference = canonical_sub_ref(&a.target);
    let changed = if remove {
        wipe_core::inbox::unsubscribe(&s, &who, &reference)?
    } else {
        wipe_core::inbox::subscribe(&s, &who, &reference)?
    };
    let verb = if remove { "unsubscribed" } else { "subscribed" };
    let noun = if changed {
        verb
    } else if remove {
        "was not subscribed"
    } else {
        "already subscribed"
    };
    out.ok(
        format!(
            "{who} {noun} {} {reference}",
            if remove { "from" } else { "to" }
        ),
        json!({ "ok": true, "identity": who, "ref": reference, "changed": changed }),
    );
    Ok(())
}

/// `wipe subscriptions` - list an identity's subscriptions.
pub fn subscriptions(out: &Out, author: Option<String>) -> Result<()> {
    let s = store()?;
    let who = identity::resolve(author)?;
    let refs = wipe_core::inbox::subscriptions_of(&s, &who)?;
    if out.json {
        out.json_value(&json!({ "identity": who, "subscriptions": refs }));
    } else if refs.is_empty() {
        out.line(format!("{who} has no subscriptions"));
    } else {
        out.line(format!("{who} subscribes to:"));
        for r in &refs {
            out.line(format!("  {r}"));
        }
    }
    Ok(())
}

/// `wipe inbox` - return unread activity on things you care about, then exit.
pub fn inbox(out: &Out, a: InboxArgs) -> Result<()> {
    let s = store()?;
    let who = identity::resolve(a.author)?;

    // Determine the lower bound: explicit --since wins; otherwise the stored
    // read-cursor; otherwise the epoch (everything).
    let since = if let Some(raw) = &a.since {
        view::parse_since(raw, Utc::now())?
    } else {
        wipe_core::inbox::read_cursor(&s, &who).unwrap_or(chrono::DateTime::<Utc>::UNIX_EPOCH)
    };

    let now = Utc::now();
    let mut events = wipe_core::inbox::inbox_with(&s, &who, since, a.all)?;
    let total = events.len();
    if a.limit > 0 {
        events.truncate(a.limit);
    }

    // --unread consumes: advance the cursor so the next call only shows newer.
    if a.unread && a.since.is_none() {
        wipe_core::inbox::write_cursor(&s, &who, now)?;
    }

    if out.json {
        out.json_value(&json!({
            "identity": who,
            "since": since.to_rfc3339(),
            "count": events.len(),
            "total": total,
            "events": events,
        }));
    } else if events.is_empty() {
        out.line(format!(
            "inbox empty for {who} (since {})",
            since.to_rfc3339()
        ));
    } else {
        if total > events.len() {
            out.line(format!(
                "{} new for {who} (showing the newest {} - `--limit 0` for all):",
                total,
                events.len()
            ));
        } else {
            out.line(format!("{} new for {who}:", events.len()));
        }
        for e in &events {
            out.line(format!(
                "  {}  {} {}  {} [{}]  {}",
                e.ts.format("%Y-%m-%d %H:%M"),
                e.kind,
                e.object,
                dim(&e.actor),
                e.reason,
                e.detail
            ));
        }
    }
    Ok(())
}

/// `wipe trash ...` - the restorable, gitignored bin for deleted tickets.
pub fn trash(out: &Out, cmd: TrashCmd) -> Result<()> {
    let s = store()?;
    let days = GlobalConfig::load().trash_retention_days();
    match cmd {
        TrashCmd::List => {
            let entries = wipe_core::trash::list_trash(&s, days, Utc::now())?;
            if out.json {
                let rows: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.ticket.id,
                            "title": e.ticket.title,
                            "list": e.list,
                            "deleted_at": e.deleted_at.to_rfc3339(),
                        })
                    })
                    .collect();
                out.json_value(&json!({ "retention_days": days, "trash": rows }));
            } else if entries.is_empty() {
                out.line("trash is empty");
            } else {
                out.line(format!("{} trashed (retention {days}d):", entries.len()));
                for e in &entries {
                    out.line(format!(
                        "  {}  {}  {}",
                        e.ticket.id,
                        dim(&e.deleted_at.format("%Y-%m-%d %H:%M").to_string()),
                        e.ticket.title
                    ));
                }
            }
        }
        TrashCmd::Restore { id } => {
            let t = wipe_core::trash::restore_ticket(&s, &id, Utc::now())?;
            out.ok(
                format!("restored {} to {}", t.id, "the board"),
                json!({ "ok": true, "id": t.id }),
            );
        }
        TrashCmd::Purge { id } => match id {
            Some(id) => {
                let removed = wipe_core::trash::purge_ticket(&s, &id)?;
                out.ok(
                    if removed {
                        format!("permanently deleted {id}")
                    } else {
                        format!("{id} was not in the trash")
                    },
                    json!({ "ok": true, "id": id, "purged": removed }),
                );
            }
            None => {
                let n = wipe_core::trash::empty(&s)?;
                out.ok(
                    format!("emptied the trash ({n} removed)"),
                    json!({ "ok": true, "purged": n }),
                );
            }
        },
    }
    Ok(())
}

/// `wipe commit` - stage and commit the board's `.wipe/` changes as one atomic,
/// wipe-attributed git commit (optionally scoped to a single ticket).
pub fn commit(out: &Out, a: CommitArgs) -> Result<()> {
    let s = store()?;
    if !wipe_core::git::is_repo(s.root()) {
        bail!("not a git repository - `wipe commit` needs the board to live inside a git repo");
    }
    let who = identity::resolve(a.author)?;
    let hash = ops::commit_board(&s, a.target.as_deref(), a.message.as_deref(), &who)?;
    // Someone who has committed the board once knows the move; retire the hint.
    let mut g = GlobalConfig::load();
    if g.commit_hint_seen != Some(true) {
        g.commit_hint_seen = Some(true);
        let _ = g.save();
    }
    match hash {
        Some(h) => out.ok(
            format!("committed {h} as {who}"),
            json!({ "ok": true, "commit": h, "author": who }),
        ),
        None => out.ok(
            "nothing to commit - the board is already up to date",
            json!({ "ok": true, "commit": Value::Null, "author": who }),
        ),
    }
    Ok(())
}

/// Whether this process was launched through the npm package's node shim
/// (marked by the patched shim; see `scripts/patch-npm-installer.mjs`).
pub(crate) fn via_npm_shim() -> bool {
    std::env::var_os("WIPE_NPM_SHIM").is_some()
}

/// `wipe doctor`
pub fn doctor(out: &Out) -> Result<()> {
    let in_board = Store::discover(".").ok();
    let git = identity::git_available();
    let author = identity::resolve_opt(None);
    let author_source = identity::source(None);
    let session = identity::session_key().is_some();
    let detected = vcs::detect(std::path::Path::new("."));
    let (board_name, tickets) = match &in_board {
        Some(s) => (Some(s.load_board()?.name), s.ticket_ids()?.len()),
        None => (None, 0),
    };
    let autocommit = in_board
        .as_ref()
        .and_then(|s| s.load_settings().ok())
        .map(|st| st.autocommit)
        .unwrap_or(false);
    let uncommitted = in_board
        .as_ref()
        .filter(|s| wipe_core::git::is_repo(s.root()))
        .map(board_uncommitted)
        .unwrap_or(0);
    let exe = std::env::current_exe()
        .ok()
        .map(|p| clean_path(&p))
        .unwrap_or_default();

    let mut warnings: Vec<String> = Vec::new();
    if author.is_none() {
        warnings.push(
            "no identity chosen - writes are refused until you run `wipe identity use <id>` \
             (or set $WIPE_AGENT / pass --agentid)"
                .into(),
        );
    }
    if cfg!(windows) && via_npm_shim() {
        warnings.push(format!(
            "running through the npm shim: when it is reached via cmd.exe (wipe.cmd, \
             shell=True), multi-line arguments are cut at the first line break with no error. \
             pass long text with --body-file <path> or --body - (stdin), or call the binary \
             directly: {exe}"
        ));
    }
    if uncommitted > 0 && !autocommit {
        warnings.push(format!(
            "{uncommitted} board file(s) changed but not committed - `wipe commit` records them \
             as one wipe-attributed commit (or `wipe config set board.autocommit true`)"
        ));
    }

    if out.json {
        out.json_value(&json!({
            "in_board": in_board.is_some(),
            "board": board_name,
            "tickets": tickets,
            "git_available": git,
            "vcs": detected.name(),
            "identity": author,
            "identity_source": author_source,
            "session_detected": session,
            "npm_shim": via_npm_shim(),
            "binary": exe,
            "autocommit": autocommit,
            "uncommitted_board_files": uncommitted,
            "warnings": warnings,
            "version": env!("CARGO_PKG_VERSION"),
        }));
    } else {
        let mark = |b: bool| if b { "✓" } else { "✗" };
        println!("wipe {}  {}", env!("CARGO_PKG_VERSION"), dim(&exe));
        println!(
            "{} inside a board{}",
            mark(in_board.is_some()),
            board_name
                .map(|n| format!(": {n} ({tickets} tickets)"))
                .unwrap_or_default()
        );
        println!("{} git available", mark(git));
        println!("  vcs: {}", detected.name());
        match &author {
            Some(a) => println!(
                "{} identity: {a}  {}",
                mark(true),
                dim(&format!("({author_source})"))
            ),
            None => println!("{} identity: none chosen", mark(false)),
        }
        println!(
            "  session: {}",
            if session {
                "detected (`wipe identity use` binds to it)"
            } else {
                "not detectable - use $WIPE_AGENT or --agentid"
            }
        );
        for w in &warnings {
            println!("! {w}");
        }
    }
    Ok(())
}

/// How many files under `.wipe/` differ from the last commit (0 when clean or
/// not in git).
fn board_uncommitted(s: &Store) -> usize {
    wipe_core::git::changed_count(s.root(), ".wipe").unwrap_or(0)
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
            d.daemon.expose = g.default_expose.unwrap_or(d.daemon.expose);
            d
        }
    };
    let port = args.port.unwrap_or(settings.daemon.port);
    let expose = if args.local {
        Exposure::Local
    } else if args.tailscale {
        Exposure::Tailscale
    } else if let Some(e) = &args.expose {
        parse_expose(e)?
    } else {
        settings.daemon.expose
    };
    let host = args
        .host
        .as_deref()
        .map(|h| {
            h.parse::<std::net::IpAddr>().map_err(|_| {
                anyhow!("--host must be an IP address (e.g. 0.0.0.0, 192.168.1.20, ::), got `{h}`")
            })
        })
        .transpose()?;

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
        expose,
        host,
        qr: !args.no_qr && std::io::stdout().is_terminal(),
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

fn print_ticket_human(
    t: &wipe_core::model::Ticket,
    list_id: Option<&str>,
    activity: bool,
    commits: &[wipe_core::git::CommitInfo],
) {
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
    let blockers: Vec<&str> = t.blocked_by().collect();
    if !blockers.is_empty() {
        println!("  {}", dim(&format!("blocked by: {}", blockers.join(", "))));
    }
    if !t.body.is_empty() {
        println!("\n{}", t.body);
    }
    if let Some(o) = &t.original {
        println!(
            "\n{}",
            dim(&format!(
                "original note by {} (kept when {} first rewrote it):",
                o.author, o.rewritten_by
            ))
        );
        println!("  {}", o.title);
        for line in o.body.lines() {
            println!("  {line}");
        }
    }
    for (name, items) in [
        ("checklist", &t.checklist),
        ("acceptance criteria", &t.acceptance),
    ] {
        if !items.is_empty() {
            let done = items.iter().filter(|i| i.done).count();
            println!("\n{}", dim(&format!("{name} ({done}/{}):", items.len())));
            for i in items {
                let box_ = if i.done { "[x]" } else { "[ ]" };
                println!("  {box_} {} {}", id_style(&i.id), i.text);
            }
        }
    }
    if !t.comments.is_empty() {
        println!();
        print_comments(t);
    }
    if activity && !t.activity.is_empty() {
        println!("\n{}", dim(&format!("{} activity:", t.activity.len())));
        for a in &t.activity {
            println!(
                "  {}  {} {} {}",
                dim(&a.ts.format("%Y-%m-%d %H:%M").to_string()),
                a.actor,
                a.kind,
                a.detail
            );
        }
    }
    if !commits.is_empty() {
        println!(
            "\n{}",
            dim(&format!("{} commit(s) mention {}:", commits.len(), t.id))
        );
        for c in commits {
            println!(
                "  {}  {}  {}",
                id_style(&c.short),
                c.subject,
                dim(&c.author_name)
            );
        }
    }
}

/// A ticket's comment thread, one block per comment with its full body.
fn print_comments(t: &wipe_core::model::Ticket) {
    if t.comments.is_empty() {
        println!("{} has no comments", t.id);
        return;
    }
    println!("{}", dim(&format!("{} comment(s):", t.comments.len())));
    for c in &t.comments {
        println!(
            "  {} {} {}",
            id_style(&c.id),
            c.author,
            dim(&c.created.format("%Y-%m-%d %H:%M").to_string())
        );
        for line in c.body.lines() {
            println!("    {line}");
        }
    }
}
