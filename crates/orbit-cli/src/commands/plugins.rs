use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::plugin::{self, InstallMethod, Plugin, PluginState};
use std::{
    io::{self, Write},
    process::Command,
};

#[derive(Debug, Args)]
pub struct PluginsArgs {
    #[command(subcommand)]
    pub command: Option<PluginsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum PluginsCommand {
    /// List all available plugins with their install and MCP status
    List,
    /// Install a plugin
    Install {
        /// Plugin name (from `orbit plugins list`)
        name: String,
        /// Install method to use (pip, npm, cargo, brew…)
        #[arg(long, short)]
        method: Option<String>,
        /// Accept defaults without prompting
        #[arg(long, short)]
        yes: bool,
    },
    /// Enable a plugin — registers its MCP servers in all orbit sessions
    Enable {
        /// Plugin name
        name: String,
    },
    /// Disable a plugin — removes its MCP servers from orbit sessions
    Disable {
        /// Plugin name
        name: String,
    },
    /// Show detailed information about a plugin
    Info {
        /// Plugin name
        name: String,
    },
    /// Wrap an AI engine with a plugin (if the plugin supports wrapping)
    Wrap {
        /// Plugin name
        name: String,
        /// Engine to wrap (default: reads from orbit config)
        #[arg(long)]
        engine: Option<String>,
    },
    /// Unwrap an AI engine previously wrapped by a plugin
    Unwrap {
        /// Plugin name
        name: String,
        /// Engine to unwrap
        #[arg(long)]
        engine: Option<String>,
    },
}

pub fn run(args: PluginsArgs) -> Result<()> {
    match args.command.unwrap_or(PluginsCommand::List) {
        PluginsCommand::List => list(),
        PluginsCommand::Install { name, method, yes } => install(&name, method.as_deref(), yes),
        PluginsCommand::Enable { name } => enable(&name),
        PluginsCommand::Disable { name } => disable(&name),
        PluginsCommand::Info { name } => info(&name),
        PluginsCommand::Wrap { name, engine } => wrap(&name, engine.as_deref()),
        PluginsCommand::Unwrap { name, engine } => unwrap_engine(&name, engine.as_deref()),
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

fn list() -> Result<()> {
    let plugins = plugin::load_all();
    let state = PluginState::load();

    if plugins.is_empty() {
        println!("No plugins available.");
        println!("Drop a .toml file into ~/.config/orbit/plugins/ to add custom plugins.");
        return Ok(());
    }

    println!("plugins\n");

    let name_w = plugins.iter().map(|p| p.name.len()).max().unwrap_or(8).max(8);
    let cat_w = plugins
        .iter()
        .map(|p| p.category.len())
        .max()
        .unwrap_or(10)
        .max(10);

    for p in &plugins {
        let installed = p.is_installed();
        let enabled = state.is_enabled(&p.name);

        // ● = installed + MCP enabled  ✓ = installed  ○ = not installed
        let status = match (installed, enabled && p.has_mcp()) {
            (_, true) => "\x1b[32m●\x1b[0m",
            (true, _) => "\x1b[32m✓\x1b[0m",
            _ => "\x1b[33m○\x1b[0m",
        };

        let mcp_tag = if p.has_mcp() {
            if enabled {
                "  \x1b[32m[mcp: active]\x1b[0m"
            } else {
                "  \x1b[2m[mcp: inactive]\x1b[0m"
            }
        } else {
            ""
        };

        println!(
            "  {status}  {name:<name_w$}  \x1b[2m({cat:<cat_w$})\x1b[0m  {desc}{mcp_tag}",
            name = p.name,
            cat = p.category,
            desc = p.description,
            name_w = name_w,
            cat_w = cat_w,
        );

        if !installed {
            if let Some(m) = p.best_install_method() {
                println!(
                    "     {blank:<name_w$}   install: {}",
                    m.cmd.join(" "),
                    blank = "",
                    name_w = name_w,
                );
            }
        } else if p.has_mcp() && !enabled {
            println!(
                "     {blank:<name_w$}   enable:  orbit plugins enable {}",
                p.name,
                blank = "",
                name_w = name_w,
            );
        }
    }

    println!();

    let installed_count = plugins.iter().filter(|p| p.is_installed()).count();
    let enabled_count = plugins
        .iter()
        .filter(|p| p.has_mcp() && state.is_enabled(&p.name))
        .count();
    let mcp_count = plugins.iter().filter(|p| p.has_mcp()).count();

    print!(
        "  {}/{} installed",
        installed_count,
        plugins.len()
    );
    if mcp_count > 0 {
        print!("  ·  {}/{} MCP active", enabled_count, mcp_count);
    }
    println!("  ·  orbit plugins install/enable <name>");

    Ok(())
}

// ── install ───────────────────────────────────────────────────────────────────

fn install(name: &str, method_name: Option<&str>, yes: bool) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!(
            "plugin not found: {name}\nRun `orbit plugins list` to see available plugins."
        )
    };

    if plugin.is_installed() {
        println!("  \x1b[32m✓\x1b[0m  {name} is already installed.");
        return Ok(());
    }

    println!();
    println!("  {name}  —  {}", plugin.description);
    println!();

    let method = if let Some(mn) = method_name {
        plugin
            .install_method_by_name(mn)
            .ok_or_else(|| {
                let available: Vec<_> = plugin.install.iter().map(|m| m.method.as_str()).collect();
                anyhow::anyhow!(
                    "unknown method '{mn}' for plugin '{name}'\navailable: {}",
                    available.join(", ")
                )
            })?
    } else if plugin.install.len() == 1 || yes {
        plugin
            .best_install_method()
            .ok_or_else(|| anyhow::anyhow!("no install method defined for plugin '{name}'"))?
    } else {
        pick_install_method(&plugin)?
    };

    run_install(name, method)?;

    if plugin.has_mcp() {
        println!();
        println!(
            "  \x1b[2mRun `orbit plugins enable {name}` to activate MCP servers.\x1b[0m"
        );
    }

    Ok(())
}

fn pick_install_method(plugin: &Plugin) -> Result<&InstallMethod> {
    println!("  Available install methods:");
    println!();
    for (i, m) in plugin.install.iter().enumerate() {
        println!("    {})  {}  —  {}", i + 1, m.label, m.cmd.join(" "));
    }
    println!();

    loop {
        print!("  Method [1]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();

        let n: usize = if trimmed.is_empty() {
            1
        } else {
            match trimmed.parse() {
                Ok(n) => n,
                Err(_) => {
                    println!("  Invalid choice.");
                    continue;
                }
            }
        };

        if n >= 1 && n <= plugin.install.len() {
            return Ok(&plugin.install[n - 1]);
        }
        println!("  Enter a number between 1 and {}.", plugin.install.len());
    }
}

fn run_install(name: &str, method: &InstallMethod) -> Result<()> {
    if method.cmd.is_empty() {
        bail!("install command is empty");
    }

    println!("  Installing {name} via {}…", method.label);
    println!("  $ {}", method.cmd.join(" "));
    println!();

    let status = Command::new(&method.cmd[0])
        .args(&method.cmd[1..])
        .status()?;

    if status.success() {
        println!();
        println!("  \x1b[32m✓\x1b[0m  Installed successfully.");
    } else {
        println!();
        println!("  \x1b[31m✗\x1b[0m  Install failed — run manually:");
        println!("     {}", method.cmd.join(" "));
    }

    Ok(())
}

// ── enable ────────────────────────────────────────────────────────────────────

fn enable(name: &str) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!(
            "plugin not found: {name}\nRun `orbit plugins list` to see available plugins."
        )
    };

    let mut state = PluginState::load();

    if state.is_enabled(name) {
        println!("  \x1b[32m✓\x1b[0m  {name} is already enabled.");
        if plugin.has_mcp() {
            let names: Vec<_> = plugin.mcp.iter().map(|m| m.name.as_str()).collect();
            println!("     MCP: {}", names.join(", "));
        }
        return Ok(());
    }

    if !plugin.is_installed() {
        println!("  \x1b[33m!\x1b[0m  {name} is not installed.");
        println!(
            "     Run `orbit plugins install {name}` first, then enable."
        );
        println!();
        println!("     Registering anyway — MCP may not work until the tool is installed.");
        println!();
    }

    state.enable(name);
    state.save()?;

    if plugin.has_mcp() {
        plugin::add_plugin_mcps(&plugin)?;
        let mcp_names: Vec<_> = plugin.mcp.iter().map(|m| m.name.as_str()).collect();
        println!("  \x1b[32m●\x1b[0m  {name} enabled");
        println!("     MCP registered: {}", mcp_names.join(", "));
        println!(
            "     Config: {}",
            plugin::plugins_mcp_path().display()
        );
        println!("     Active in new orbit sessions.");
    } else {
        println!("  \x1b[32m✓\x1b[0m  {name} enabled.");
    }

    Ok(())
}

// ── disable ───────────────────────────────────────────────────────────────────

fn disable(name: &str) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!(
            "plugin not found: {name}\nRun `orbit plugins list` to see available plugins."
        )
    };

    let mut state = PluginState::load();

    if !state.is_enabled(name) {
        println!("  {name} is not enabled.");
        return Ok(());
    }

    state.disable(name);
    state.save()?;

    if plugin.has_mcp() {
        plugin::remove_plugin_mcps(&plugin)?;
        let mcp_names: Vec<_> = plugin.mcp.iter().map(|m| m.name.as_str()).collect();
        println!(
            "  \x1b[32m✓\x1b[0m  {name} disabled — MCP removed: {}",
            mcp_names.join(", ")
        );
    } else {
        println!("  \x1b[32m✓\x1b[0m  {name} disabled.");
    }

    Ok(())
}

// ── info ──────────────────────────────────────────────────────────────────────

fn info(name: &str) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!(
            "plugin not found: {name}\nRun `orbit plugins list` to see available plugins."
        )
    };

    let state = PluginState::load();
    let enabled = state.is_enabled(name);
    let installed = plugin.is_installed();

    let status_str = match (installed, enabled && plugin.has_mcp()) {
        (true, true) => "\x1b[32minstalled · MCP active\x1b[0m",
        (true, false) if plugin.has_mcp() => "\x1b[32minstalled\x1b[0m · MCP inactive",
        (true, _) => "\x1b[32minstalled\x1b[0m",
        _ => "\x1b[33mnot installed\x1b[0m",
    };

    println!();
    println!("  \x1b[1m{}\x1b[0m", plugin.name);
    println!();
    println!("  description   {}", plugin.description);
    println!("  category      {}", plugin.category);
    if let Some(url) = &plugin.url {
        println!("  url           {url}");
    }
    println!("  status        {status_str}");

    if !plugin.install.is_empty() {
        println!();
        println!("  install");
        for m in &plugin.install {
            println!("    {:<8}  {}", m.method, m.cmd.join(" "));
        }
    }

    if let Some(auth) = &plugin.auth {
        println!();
        println!("  auth");
        println!("    {}", auth.hint);
    }

    if !plugin.mcp.is_empty() {
        println!();
        println!("  mcp servers");
        for m in &plugin.mcp {
            let mut cmd_parts = vec![m.command.clone()];
            cmd_parts.extend(m.args.iter().cloned());
            let label = m.label.as_deref().unwrap_or(&m.name);
            println!("    {}  —  {}", label, cmd_parts.join(" "));
        }
        if enabled {
            println!("    \x1b[32m[active]\x1b[0m  {}", plugin::plugins_mcp_path().display());
        } else {
            println!("    \x1b[2m[inactive — run: orbit plugins enable {name}]\x1b[0m");
        }
    }

    if let Some(wrap) = &plugin.wrap {
        println!();
        println!("  wrap");
        println!("    {}", wrap.cmd_template);
        if let Some(unwrap) = &wrap.unwrap_cmd_template {
            println!("    undo: {unwrap}");
        }
        println!("    engines: {}", wrap.engines.join(", "));
    }

    println!();
    Ok(())
}

// ── wrap / unwrap ─────────────────────────────────────────────────────────────

fn wrap(name: &str, engine: Option<&str>) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}")
    };

    if !plugin.is_installed() {
        bail!("{name} is not installed — run `orbit plugins install {name}` first");
    }

    let Some(wrap_spec) = &plugin.wrap else {
        bail!("plugin '{name}' does not support wrapping");
    };

    let engine = resolve_engine(engine, &wrap_spec.engines)?;
    let cmd = wrap_spec.cmd_template.replace("{engine}", &engine);

    println!("  Running: {cmd}");
    run_shell_cmd(&cmd)
}

fn unwrap_engine(name: &str, engine: Option<&str>) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}")
    };

    let Some(wrap_spec) = &plugin.wrap else {
        bail!("plugin '{name}' does not support wrapping");
    };

    let Some(unwrap_tmpl) = &wrap_spec.unwrap_cmd_template else {
        bail!("plugin '{name}' does not define an unwrap command");
    };

    let engine = resolve_engine(engine, &wrap_spec.engines)?;
    let cmd = unwrap_tmpl.replace("{engine}", &engine);

    println!("  Running: {cmd}");
    run_shell_cmd(&cmd)
}

fn resolve_engine(engine: Option<&str>, supported: &[String]) -> Result<String> {
    if let Some(e) = engine {
        if !supported.is_empty() && !supported.iter().any(|s| s == e) {
            bail!(
                "engine '{e}' not supported by this plugin\nsupported: {}",
                supported.join(", ")
            );
        }
        return Ok(e.to_string());
    }

    let cfg = orbit_core::user_config::UserConfig::load();
    let default = cfg.engine.default.clone();

    if !supported.is_empty() && !supported.iter().any(|s| *s == default) {
        bail!(
            "default engine '{default}' not supported by this plugin\nsupported: {}\nPass --engine <name> to override.",
            supported.join(", ")
        );
    }

    Ok(default)
}

fn run_shell_cmd(cmd: &str) -> Result<()> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        bail!("empty command");
    }
    let status = Command::new(parts[0]).args(&parts[1..]).status()?;
    if !status.success() {
        bail!("command exited with status {status}");
    }
    Ok(())
}

// ── doctor helper (called from doctor.rs) ────────────────────────────────────

pub fn print_plugins_section() {
    let plugins = plugin::load_all();

    if plugins.is_empty() {
        return;
    }

    let state = PluginState::load();

    println!("\x1b[1mplugins\x1b[0m");
    for p in &plugins {
        let installed = p.is_installed();
        let enabled = state.is_enabled(&p.name);

        if installed {
            if p.has_mcp() && enabled {
                println!(
                    "  \x1b[32m●\x1b[0m  {}  \x1b[2m[mcp: active]\x1b[0m",
                    p.name
                );
            } else if p.has_mcp() {
                println!(
                    "  \x1b[32m✓\x1b[0m  {}  \x1b[2m[mcp: inactive — orbit plugins enable {}]\x1b[0m",
                    p.name, p.name
                );
            } else {
                println!("  \x1b[32m✓\x1b[0m  {}", p.name);
            }
        } else {
            println!("  \x1b[33m○\x1b[0m  {}  — not installed", p.name);
            println!(
                "      \x1b[2minstall: orbit plugins install {}\x1b[0m",
                p.name
            );
        }
    }
    println!();
}

// ── setup helper (called from setup.rs) ──────────────────────────────────────

pub fn setup_plugins(yes: bool) -> Result<()> {
    let plugins = plugin::load_all();
    let uninstalled: Vec<_> = plugins.iter().filter(|p| !p.is_installed()).collect();

    if uninstalled.is_empty() {
        return Ok(());
    }

    println!("  Checking plugins...");
    println!();

    for p in &uninstalled {
        println!("  \x1b[33m○\x1b[0m  {}  — not installed", p.name);
        println!("      \x1b[2m{}\x1b[0m", p.description);

        let should_install = if yes {
            false
        } else {
            confirm(&format!("    Install {}?", p.name), false)?
        };

        if should_install {
            if let Some(m) = p.best_install_method() {
                println!("    Installing {}...", p.name);
                let status = Command::new(&m.cmd[0]).args(&m.cmd[1..]).status();
                match status {
                    Ok(s) if s.success() => {
                        println!("    \x1b[32m✓\x1b[0m done");
                        if p.has_mcp() {
                            println!(
                                "      Run `orbit plugins enable {}` to activate MCP servers.",
                                p.name
                            );
                        }
                    }
                    _ => println!(
                        "    \x1b[31m✗\x1b[0m failed — run: {}",
                        m.cmd.join(" ")
                    ),
                }
            }
        }

        println!();
    }

    Ok(())
}

fn confirm(question: &str, default: bool) -> Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    print!("  {question} [{hint}]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    })
}
