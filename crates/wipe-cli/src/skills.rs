//! Installing the agent `SKILL.md` into an agent skills directory.
//!
//! Follows the Agent Skills layout `<base>/skills/<name>/SKILL.md`, where `<base>`
//! is a `.claude` directory (Claude Code) or a `.agents` directory (the cross-tool
//! Agent Skills convention), located in the project (default) or the user's home
//! (`--global`). Skills placed there are auto-discovered by the respective tools.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::args::SkillInstallArgs;

/// Which skills-directory convention to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// `.claude/skills/` - Claude Code.
    Claude,
    /// `.agents/skills/` - the cross-tool Agent Skills convention.
    Agents,
}

impl Target {
    /// The dot-directory name for this convention.
    fn dir_name(self) -> &'static str {
        match self {
            Target::Claude => ".claude",
            Target::Agents => ".agents",
        }
    }

    /// A short machine slug (for `--json`).
    pub fn slug(self) -> &'static str {
        match self {
            Target::Claude => "claude",
            Target::Agents => "agents",
        }
    }

    /// A human label for messages.
    pub fn label(self) -> &'static str {
        match self {
            Target::Claude => "Claude Code (.claude)",
            Target::Agents => "cross-tool agents (.agents)",
        }
    }

    fn parse(s: &str) -> Result<Target> {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" | ".claude" => Ok(Target::Claude),
            "agents" | "agent" | ".agents" | ".agent" => Ok(Target::Agents),
            other => bail!("unknown skill target '{other}' (use `claude` or `agents`)"),
        }
    }
}

/// A resolved install location.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Which convention was chosen.
    pub target: Target,
    /// Whether this targets the user's home directory.
    pub global: bool,
    /// The `skills/wipe/` directory that will be created.
    pub skill_dir: PathBuf,
    /// The `SKILL.md` file that will be written.
    pub file: PathBuf,
}

fn home() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .context("cannot resolve your home directory")
}

fn make_plan(target: Target, global: bool, base: PathBuf) -> Plan {
    let skill_dir = base.join("skills").join("wipe");
    let file = skill_dir.join("SKILL.md");
    Plan {
        target,
        global,
        skill_dir,
        file,
    }
}

/// Prefer whichever convention already exists under `root`; default to Claude.
fn detect_target(root: &Path) -> Target {
    if root.join(".claude").is_dir() {
        Target::Claude
    } else if root.join(".agents").is_dir() {
        Target::Agents
    } else {
        Target::Claude
    }
}

/// Decide where to install from the CLI flags plus auto-detection.
pub fn plan(args: &SkillInstallArgs) -> Result<Plan> {
    // An explicit --dir is treated as the base that receives a `skills/` dir.
    if let Some(dir) = &args.dir {
        let target = match &args.target {
            Some(t) => Target::parse(t)?,
            None => Target::Claude,
        };
        return Ok(make_plan(target, args.global, dir.clone()));
    }
    let root = if args.global {
        home()?
    } else {
        std::env::current_dir()?
    };
    let target = match &args.target {
        Some(t) => Target::parse(t)?,
        None => detect_target(&root),
    };
    let base = root.join(target.dir_name());
    Ok(make_plan(target, args.global, base))
}

/// Write `SKILL.md` at the planned location. Refuses to overwrite unless `force`.
pub fn install(plan: &Plan, contents: &str, force: bool) -> Result<()> {
    if plan.file.exists() && !force {
        bail!(
            "{} already exists (pass --force to overwrite)",
            plan.file.display()
        );
    }
    std::fs::create_dir_all(&plan.skill_dir)
        .with_context(|| format!("creating {}", plan.skill_dir.display()))?;
    std::fs::write(&plan.file, contents)
        .with_context(|| format!("writing {}", plan.file.display()))?;
    Ok(())
}
