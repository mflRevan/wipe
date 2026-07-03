//! Implementations of every `wipe` subcommand. Each returns `anyhow::Result<()>`
//! and prints through [`Out`], so the caller only has to map errors to exit codes.

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

use wipe_core::model::Exposure;
use wipe_core::ops::{self, NewTicket, TicketPatch};
use wipe_core::Store;

use crate::args::*;
use crate::identity;
use crate::output::{dim, id_style, Out};

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

/// `wipe init`
pub fn init(out: &Out, args: InitArgs) -> Result<()> {
    std::fs::create_dir_all(&args.path)
        .with_context(|| format!("creating {}", args.path.display()))?;
    let name = match args.name {
        Some(n) => n,
        None => std::fs::canonicalize(&args.path)?
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("board")
            .to_string(),
    };
    let store = Store::init(&args.path, &name, Utc::now())?;
    let path = clean_path(&store.wipe_dir());
    out.ok(
        format!("initialized wipe board '{name}' at {path}"),
        json!({ "ok": true, "name": name, "path": path }),
    );
    Ok(())
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
            out.ok(format!("created {} — {}", t.id, t.title), to_value(&t));
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
fn guess_mime(name: &str) -> &'static str {
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

/// `wipe config ...`
pub fn config(out: &Out, cmd: ConfigCmd) -> Result<()> {
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
                println!("board.name    {}", board.name);
                println!("daemon.port   {}", settings.daemon.port);
                println!("daemon.expose {:?}", settings.daemon.expose);
            }
        }
        ConfigCmd::Get { key } => {
            let settings = s.load_settings()?;
            let value = match key.as_str() {
                "daemon.port" => json!(settings.daemon.port),
                "daemon.expose" => json!(format!("{:?}", settings.daemon.expose).to_lowercase()),
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

/// `wipe doctor`
pub fn doctor(out: &Out) -> Result<()> {
    let in_board = Store::discover(".").ok();
    let git = identity::git_available();
    let author = identity::resolve(None);
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
            "author": author,
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
        println!("  identity: {author}");
    }
    Ok(())
}

/// `wipe skill`
pub fn skill(out: &Out) -> Result<()> {
    if out.json {
        out.json_value(&json!({ "skill": SKILL }));
    } else {
        print!("{SKILL}");
    }
    Ok(())
}

/// `wipe serve` — start the local daemon serving the board UI + API.
pub fn serve(out: &Out, args: ServeArgs) -> Result<()> {
    let s = store()?;
    let settings = s.load_settings()?;
    let port = args.port.unwrap_or(settings.daemon.port);
    let cfg = wipe_daemon::ServeConfig {
        root: s.root().to_path_buf(),
        port,
        expose: settings.daemon.expose,
        open: args.open,
    };
    out.line(format!(
        "starting wipe UI for '{}' on port {port}…",
        s.load_board()?.name
    ));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting async runtime")?;
    rt.block_on(wipe_daemon::serve(cfg))?;
    Ok(())
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
