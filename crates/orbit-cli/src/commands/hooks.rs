use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::engine_hook::{self, EngineHookState};

use crate::output::truncate_desc;

#[derive(Debug, Args)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub command: Option<HooksCommand>,
}

#[derive(Debug, Subcommand)]
pub enum HooksCommand {
    /// List all available engine hooks with their enabled status
    List,
    /// Enable an engine hook — injects it into Claude Code sessions at launch
    Enable {
        /// Hook name (from `orbit hooks list`)
        name: String,
    },
    /// Disable an engine hook
    Disable {
        /// Hook name
        name: String,
    },
    /// Show detailed information about an engine hook
    Info {
        /// Hook name
        name: String,
    },
}

pub fn run(args: HooksArgs) -> Result<()> {
    match args.command.unwrap_or(HooksCommand::List) {
        HooksCommand::List => list(),
        HooksCommand::Enable { name } => enable(&name),
        HooksCommand::Disable { name } => disable(&name),
        HooksCommand::Info { name } => info(&name),
    }
}

fn list() -> Result<()> {
    let hooks = engine_hook::load_all();
    let state = EngineHookState::load();

    println!("hooks\n");
    println!("  \x1b[2mEngine hooks extend orbit sessions with automated behaviors on Claude Code events.\x1b[0m\n");

    if hooks.is_empty() {
        println!("  No engine hooks defined.");
        return Ok(());
    }

    let name_w = hooks.iter().map(|h| h.name.len()).max().unwrap_or(8).max(8);
    let cat_w = hooks.iter().map(|h| h.category.len()).max().unwrap_or(8).max(8);
    let desc_w: usize = 48;
    let sep_w = 5 + name_w + 2 + cat_w + 2 + desc_w + 2 + 12;

    println!(
        "     \x1b[2m{name:<name_w$}  {cat:<cat_w$}  {desc:<desc_w$}  status\x1b[0m",
        name = "name",
        cat = "category",
        desc = "description",
        name_w = name_w,
        cat_w = cat_w,
        desc_w = desc_w,
    );
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));

    for h in &hooks {
        let enabled = state.is_enabled(&h.name);
        let status = if enabled {
            "\x1b[32m●\x1b[0m"
        } else {
            "\x1b[2m○\x1b[0m"
        };
        let status_tag = if enabled {
            "\x1b[32m[enabled]\x1b[0m "
        } else {
            "\x1b[2m[disabled]\x1b[0m"
        };
        let bin_tag = if h.requires_binary.is_some() {
            "  \x1b[36m⚙\x1b[0m"
        } else {
            ""
        };
        let desc = truncate_desc(&h.description, desc_w);
        println!(
            "  {status}  {name:<name_w$}  \x1b[2m{cat:<cat_w$}\x1b[0m  {desc:<desc_w$}  {status_tag}{bin_tag}",
            name = h.name,
            cat = h.category,
            name_w = name_w,
            cat_w = cat_w,
            desc_w = desc_w,
        );
    }

    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));
    println!("  \x1b[2m● enabled  ○ disabled  ·  ⚙ requires binary\x1b[0m");
    println!();

    let enabled_count = hooks.iter().filter(|h| state.is_enabled(&h.name)).count();
    println!(
        "  {enabled_count}/{total} enabled  ·  orbit hooks enable/disable <name>",
        total = hooks.len()
    );

    Ok(())
}

fn enable(name: &str) -> Result<()> {
    let hooks = engine_hook::load_all();
    let hook = hooks.iter().find(|h| h.name == name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown engine hook: '{name}'. Run `orbit hooks list` to see available hooks."
        )
    })?;

    let mut state = EngineHookState::load();
    if state.is_enabled(name) {
        println!("Engine hook '{name}' is already enabled.");
        return Ok(());
    }
    state.enable(name);
    state.save()?;

    let written = engine_hook::install_scripts(hook)?;
    for path in &written {
        println!("  script → {}", path.display());
    }

    println!("Engine hook '{name}' enabled — will inject into Claude Code sessions at launch.");
    Ok(())
}

fn disable(name: &str) -> Result<()> {
    let mut state = EngineHookState::load();
    if !state.is_enabled(name) {
        println!("Engine hook '{name}' is not enabled.");
        return Ok(());
    }
    state.disable(name);
    state.save()?;
    println!("Engine hook '{name}' disabled.");
    Ok(())
}

fn info(name: &str) -> Result<()> {
    let hook =
        engine_hook::find(name).ok_or_else(|| anyhow::anyhow!("unknown engine hook: '{name}'"))?;
    let state = EngineHookState::load();

    println!("Name:        {}", hook.name);
    println!("Description: {}", hook.description);
    println!("Category:    {}", hook.category);
    println!(
        "Status:      {}",
        if state.is_enabled(&hook.name) {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some(bin) = &hook.requires_binary {
        let found = std::process::Command::new("which")
            .arg(bin)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        println!(
            "Requires:    {} ({})",
            bin,
            if found { "installed" } else { "NOT FOUND" }
        );
    }
    if !hook.events.is_empty() {
        println!("Events:");
        for ev in &hook.events {
            let async_tag = if ev.is_async { " [async]" } else { "" };
            let matcher = ev
                .matcher
                .as_deref()
                .map(|m| format!(" (matcher: {m})"))
                .unwrap_or_default();
            println!(
                "  {} → {}{}{}",
                ev.event,
                orbit_core::engine_hook::expand_home(&ev.command),
                matcher,
                async_tag
            );
        }
    }
    Ok(())
}
