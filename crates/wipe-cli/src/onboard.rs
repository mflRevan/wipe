//! The interactive `wipe init` onboarding wizard (and its non-interactive
//! counterpart). Produces a [`Plan`] describing the board to create, the daemon
//! settings to write, and whether to install the agent skill.

use anyhow::{Context, Result};
use inquire::{Confirm, CustomType, Select, Text};

use wipe_core::model::{Exposure, Settings, Starter};
use wipe_core::GlobalConfig;

use crate::skills::Target;

/// Where (if anywhere) to install the agent skill during onboarding.
#[derive(Debug, Clone, Copy)]
pub struct SkillChoice {
    pub target: Target,
    pub global: bool,
}

/// The resolved onboarding decisions.
#[derive(Debug, Clone)]
pub struct Plan {
    pub name: String,
    pub starter: Starter,
    pub port: u16,
    pub expose: Exposure,
    pub autoserve: bool,
    pub idle_timeout_secs: u64,
    pub skill: Option<SkillChoice>,
}

/// Parse a `--starter` value.
pub fn parse_starter(s: &str) -> Result<Starter> {
    match s.trim().to_ascii_lowercase().as_str() {
        "standard" | "default" => Ok(Starter::Standard),
        "lists" | "lists-only" => Ok(Starter::ListsOnly),
        "empty" | "blank" | "none" => Ok(Starter::Empty),
        other => anyhow::bail!("unknown starter '{other}' (use standard|lists|empty)"),
    }
}

fn default_port(g: &GlobalConfig) -> u16 {
    g.default_port
        .unwrap_or_else(|| Settings::default().daemon.port)
}

/// Build a plan without prompting, from flags + global defaults. Used for
/// `--yes`, non-TTY, and `--json` runs.
pub fn non_interactive(name: String, starter: Option<Starter>, g: &GlobalConfig) -> Plan {
    Plan {
        name,
        starter: starter.or(g.starter).unwrap_or_default(),
        port: default_port(g),
        expose: g.default_expose.unwrap_or_default(),
        autoserve: g.autoserve.unwrap_or(false),
        idle_timeout_secs: g.idle_timeout_secs.unwrap_or(900),
        // Non-interactive runs never touch files outside `.wipe` implicitly.
        skill: None,
    }
}

/// Run the interactive wizard. `default_starter` seeds the starter question
/// (e.g. from an explicit `--starter` flag or the user's global default).
pub fn wizard(default_name: &str, default_starter: Starter, g: &GlobalConfig) -> Result<Plan> {
    println!("\n  Let's set up your wipe board. Press Enter to accept a default.\n");

    let name = Text::new("[1/6] Board name")
        .with_default(default_name)
        .prompt()
        .map_err(cancel)?;

    let starter = match Select::new(
        "[2/6] What should the board start with?",
        vec![
            "Standard - Backlog/Todo/In Progress/Done, plus a few labels",
            "Lists only - the standard lists, no labels",
            "Empty - a blank board you fill yourself",
        ],
    )
    .with_starting_cursor(match default_starter {
        Starter::Standard => 0,
        Starter::ListsOnly => 1,
        Starter::Empty => 2,
    })
    .raw_prompt()
    .map_err(cancel)?
    .index
    {
        1 => Starter::ListsOnly,
        2 => Starter::Empty,
        _ => Starter::Standard,
    };

    let port = CustomType::<u16>::new("[3/6] Local UI port")
        .with_default(default_port(g))
        .with_error_message("Enter a port number between 0 and 65535")
        .prompt()
        .map_err(cancel)?;

    let expose = match Select::new(
        "[4/6] How should the UI be reachable?",
        vec![
            "Local only (recommended)",
            "Tailscale network",
            "Behind a reverse proxy",
        ],
    )
    .raw_prompt()
    .map_err(cancel)?
    .index
    {
        1 => Exposure::Tailscale,
        2 => Exposure::Proxy,
        _ => Exposure::None,
    };

    let autoserve = Confirm::new("[5/6] Auto-stop the UI server when no one is viewing it?")
        .with_default(g.autoserve.unwrap_or(true))
        .with_help_message("Keeps zero background overhead when the board isn't open")
        .prompt()
        .map_err(cancel)?;

    let skill = match Select::new(
        "[6/6] Install the agent skill (teaches AI agents to drive wipe)?",
        vec![
            "Project - Claude Code (.claude/skills)",
            "Project - cross-tool (.agents/skills)",
            "Global - Claude Code (~/.claude/skills)",
            "Global - cross-tool (~/.agents/skills)",
            "Skip for now (you can run `wipe skill install` later)",
        ],
    )
    .raw_prompt()
    .map_err(cancel)?
    .index
    {
        0 => Some(SkillChoice {
            target: Target::Claude,
            global: false,
        }),
        1 => Some(SkillChoice {
            target: Target::Agents,
            global: false,
        }),
        2 => Some(SkillChoice {
            target: Target::Claude,
            global: true,
        }),
        3 => Some(SkillChoice {
            target: Target::Agents,
            global: true,
        }),
        _ => None,
    };

    Ok(Plan {
        name,
        starter,
        port,
        expose,
        autoserve,
        idle_timeout_secs: g.idle_timeout_secs.unwrap_or(900),
        skill,
    })
}

/// Turn an inquire cancellation into a clean, quiet abort message.
fn cancel(e: inquire::InquireError) -> anyhow::Error {
    use inquire::InquireError::*;
    match e {
        OperationCanceled | OperationInterrupted => anyhow::anyhow!("onboarding cancelled"),
        other => anyhow::Error::new(other).context("onboarding prompt failed"),
    }
}

/// Remember the choices as global defaults so future `wipe init` runs pre-fill them.
pub fn remember(plan: &Plan) -> Result<()> {
    let mut g = GlobalConfig::load();
    g.default_port = Some(plan.port);
    g.default_expose = Some(plan.expose);
    g.autoserve = Some(plan.autoserve);
    g.idle_timeout_secs = Some(plan.idle_timeout_secs);
    g.starter = Some(plan.starter);
    if let Some(s) = &plan.skill {
        g.skill_target = Some(s.target.slug().to_string());
        g.skill_global = Some(s.global);
    }
    g.save().context("saving global config")?;
    Ok(())
}
