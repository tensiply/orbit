use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::engine_hook::{self, EngineHookState};

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

    if hooks.is_empty() {
        println!("No engine hooks defined.");
        return Ok(());
    }

    for h in &hooks {
        let status = if state.is_enabled(&h.name) {
            "[enabled] "
        } else {
            "[disabled]"
        };
        println!("{status}  {}  —  {}", h.name, h.description);
    }
    Ok(())
}

fn enable(name: &str) -> Result<()> {
    let hooks = engine_hook::load_all();
    if hooks.iter().all(|h| h.name != name) {
        bail!("unknown engine hook: '{name}'. Run `orbit hooks list` to see available hooks.");
    }
    let mut state = EngineHookState::load();
    if state.is_enabled(name) {
        println!("Engine hook '{name}' is already enabled.");
        return Ok(());
    }
    state.enable(name);
    state.save()?;
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
    let hook = engine_hook::find(name)
        .ok_or_else(|| anyhow::anyhow!("unknown engine hook: '{name}'"))?;
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
