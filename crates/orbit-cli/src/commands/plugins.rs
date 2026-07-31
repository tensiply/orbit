use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::{
    plugin::{
        self, add_instance_mcps, instance_keychain_key, remove_instance_mcps, InstallMethod,
        Plugin, PluginInstanceRecord, PluginInstances, PluginState,
    },
    secrets,
};
use std::collections::{BTreeMap, HashMap};
use std::{
    io::{self, Write},
    process::Command,
};

use crate::output::truncate_desc;

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
    /// Run a plugin executor directly (without a plan). Use `shell` for ad-hoc commands.
    ///
    /// Examples:
    ///   orbit plugins run cargo --param subcommand=check
    ///   orbit plugins run shell --param command="echo hello world"
    Run {
        /// Plugin name, or `shell` for the built-in escape hatch
        name: String,
        /// Parameter values as key=value pairs (repeatable)
        #[arg(long = "param", short = 'p', value_name = "KEY=VALUE")]
        params: Vec<String>,
    },
    /// Configure credentials for a plugin interactively
    ///
    /// Prompts for each required credential and stores it in the OS keychain.
    /// Use `orbit secret get <KEY>` to verify stored values.
    ///
    /// For multi-instance plugins (e.g. jenkins), each run adds or updates one
    /// named instance.  Use --list to see configured instances and --remove to
    /// delete one.
    ///
    /// Examples:
    ///   orbit plugins auth sonarcloud
    ///   orbit plugins auth jenkins          # add/update an instance
    ///   orbit plugins auth jenkins --list   # show configured instances
    ///   orbit plugins auth jenkins --remove prod
    Auth {
        /// Plugin name (from `orbit plugins list`)
        name: String,
        /// List configured instances (multi-instance plugins only)
        #[arg(long)]
        list: bool,
        /// Remove a named instance (multi-instance plugins only)
        #[arg(long, value_name = "INSTANCE")]
        remove: Option<String>,
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
        PluginsCommand::Run { name, params } => run_executor(&name, &params),
        PluginsCommand::Auth { name, list, remove } => auth(&name, list, remove.as_deref()),
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
    println!("  \x1b[2mPlugins extend orbit with tools, MCP servers, and session context.\x1b[0m\n");

    let name_w = plugins.iter().map(|p| p.name.len()).max().unwrap_or(8).max(8);
    let cat_w = plugins.iter().map(|p| p.category.len()).max().unwrap_or(10).max(10);
    let desc_w: usize = 50;
    let sep_w = 5 + name_w + 2 + cat_w + 2 + desc_w + 2 + 16;

    // Group by category, sorted
    let mut by_cat: BTreeMap<&str, Vec<&Plugin>> = BTreeMap::new();
    for p in &plugins {
        by_cat.entry(p.category.as_str()).or_default().push(p);
    }

    // Header
    println!(
        "     \x1b[2m{name:<name_w$}  {cat:<cat_w$}  {desc:<desc_w$}  tags\x1b[0m",
        name = "name",
        cat = "category",
        desc = "description",
        name_w = name_w,
        cat_w = cat_w,
        desc_w = desc_w,
    );
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));

    let mut pending: Vec<String> = Vec::new();

    for group in by_cat.values() {
        for p in group {
            let installed = p.is_installed();
            let enabled = state.is_enabled(&p.name);

            let status = match (installed, enabled && p.has_mcp()) {
                (_, true) => "\x1b[32m●\x1b[0m",
                (true, _) => "\x1b[32m✓\x1b[0m",
                _ => "\x1b[33m○\x1b[0m",
            };

            let desc = truncate_desc(&p.description, desc_w);

            let mcp_tag = if p.has_mcp() {
                if enabled { "\x1b[32mmcp ●\x1b[0m" } else { "\x1b[2mmcp ○\x1b[0m" }
            } else {
                ""
            };
            let exec_tag = if p.executor.is_some() { "\x1b[36m⚙\x1b[0m" } else { "" };
            let tags = match (!mcp_tag.is_empty(), !exec_tag.is_empty()) {
                (true, true) => format!("{mcp_tag}  {exec_tag}"),
                (true, false) => mcp_tag.to_string(),
                (false, true) => exec_tag.to_string(),
                (false, false) => String::new(),
            };

            println!(
                "  {status}  {name:<name_w$}  \x1b[2m{cat:<cat_w$}\x1b[0m  {desc:<desc_w$}  {tags}",
                name = p.name,
                cat = p.category,
                name_w = name_w,
                cat_w = cat_w,
                desc_w = desc_w,
            );

            let needs_install = !installed && p.best_install_method().is_some();
            let needs_auth = p.auth.as_ref().is_some_and(|a| {
                !a.vars.is_empty() && a.vars.iter().any(|v| secrets::keychain_get(&v.name).is_err())
            });
            let needs_enable = p.has_mcp() && !enabled;

            if needs_install && let Some(m) = p.best_install_method() {
                pending.push(format!(
                    "  orbit plugins install {:<name_w$}  \x1b[2m# {}\x1b[0m",
                    p.name,
                    m.cmd.join(" "),
                    name_w = name_w,
                ));
            }
            if !needs_install && needs_auth {
                pending.push(format!("  orbit plugins auth    {}", p.name));
            }
            if !needs_install && needs_enable {
                pending.push(format!("  orbit plugins enable  {}", p.name));
            }
        }
    }

    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));
    println!("  \x1b[2m● installed+MCP active  ✓ installed  ○ not installed  ·  mcp ● active  mcp ○ inactive  ⚙ executor\x1b[0m");

    if !pending.is_empty() {
        println!("\n  \x1b[2mPending Actions:\x1b[0m");
        for h in &pending {
            println!("{h}");
        }
    }

    println!();

    let installed_count = plugins.iter().filter(|p| p.is_installed()).count();
    let enabled_count = plugins
        .iter()
        .filter(|p| p.has_mcp() && state.is_enabled(&p.name))
        .count();
    let mcp_count = plugins.iter().filter(|p| p.has_mcp()).count();

    print!("  {}/{} installed", installed_count, plugins.len());
    if mcp_count > 0 {
        print!("  ·  {}/{} MCP active", enabled_count, mcp_count);
    }
    println!("  ·  orbit plugins install/enable <name>");

    Ok(())
}

// ── install ───────────────────────────────────────────────────────────────────

fn install(name: &str, method_name: Option<&str>, yes: bool) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}\nRun `orbit plugins list` to see available plugins.")
    };

    if plugin.is_installed() {
        return install_missing_components(&plugin, yes);
    }

    println!();
    println!("  {name}  —  {}", plugin.description);
    println!();

    let method = if let Some(mn) = method_name {
        plugin.install_method_by_name(mn).ok_or_else(|| {
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

    run_install(&plugin, method)?;

    if plugin.has_mcp() {
        println!();
        println!("  \x1b[2mRun `orbit plugins enable {name}` to activate MCP servers.\x1b[0m");
    }

    Ok(())
}

fn install_missing_components(plugin: &Plugin, yes: bool) -> Result<()> {
    let name = &plugin.name;

    let missing: Vec<&InstallMethod> = plugin
        .install
        .iter()
        .filter(|m| !m.is_step_installed())
        .collect();

    if missing.is_empty() {
        println!("  \x1b[32m✓\x1b[0m  {name} is already installed — all components present.");
        return Ok(());
    }

    println!();
    println!("  \x1b[32m✓\x1b[0m  {name} core is installed. Missing packages:");
    println!();
    for m in &missing {
        println!("    \x1b[33m○\x1b[0m  {}  —  {}", m.label, m.cmd.join(" "));
    }
    println!();

    let should_install = if yes {
        true
    } else {
        confirm(&format!("  Install {} missing package(s)?", missing.len()), true)?
    };

    if should_install {
        for m in &missing {
            run_install(plugin, m)?;
            println!();
        }
    }

    if plugin.has_mcp() && !PluginState::load().is_enabled(name) {
        println!("  \x1b[2mRun `orbit plugins enable {name}` to activate MCP servers.\x1b[0m");
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

fn run_install(plugin: &Plugin, method: &InstallMethod) -> Result<()> {
    if method.cmd.is_empty() {
        bail!("install command is empty");
    }

    // For venv-based plugins, ensure the orbit Python venv exists and redirect
    // pip/pip3 to the venv's own pip so the package stays isolated.
    let cmd: Vec<String> =
        if plugin.use_orbit_venv && (method.method == "pip" || method.method == "pip3") {
            orbit_core::venv::ensure_venv()?;
            let venv_pip = orbit_core::venv::venv_bin("pip");
            let mut c = method.cmd.clone();
            c[0] = venv_pip.to_string_lossy().to_string();
            c
        } else {
            method.cmd.clone()
        };

    println!("  Installing {} via {}…", plugin.name, method.label);
    println!("  $ {}", cmd.join(" "));
    println!();

    let status = Command::new(&cmd[0]).args(&cmd[1..]).status()?;

    if status.success() {
        println!();
        println!("  \x1b[32m✓\x1b[0m  Installed successfully.");
    } else {
        println!();
        println!("  \x1b[31m✗\x1b[0m  Install failed — run manually:");
        println!("     {}", cmd.join(" "));
    }

    Ok(())
}

// ── enable ────────────────────────────────────────────────────────────────────

fn enable(name: &str) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}\nRun `orbit plugins list` to see available plugins.")
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
        println!("     Run `orbit plugins install {name}` first, then enable.");
        println!();
        println!("     Registering anyway — MCP may not work until the tool is installed.");
        println!();
    }

    state.enable(name);
    state.save()?;

    if plugin.has_mcp() {
        plugin::add_plugin_mcps(&plugin)?;
        let instances = PluginInstances::load();
        add_instance_mcps(&plugin, &instances)?;

        let mut mcp_names: Vec<String> = plugin.mcp.iter().map(|m| m.name.clone()).collect();
        for r in instances.for_plugin(name) {
            mcp_names.push(format!("{}-{}", name, r.name));
        }
        println!("  \x1b[32m●\x1b[0m  {name} enabled");
        if !mcp_names.is_empty() {
            println!("     MCP registered: {}", mcp_names.join(", "));
        }
        println!("     Config: {}", plugin::plugins_mcp_path().display());
        println!("     Active in new orbit sessions.");
    } else {
        println!("  \x1b[32m✓\x1b[0m  {name} enabled.");
    }

    Ok(())
}

// ── disable ───────────────────────────────────────────────────────────────────

fn disable(name: &str) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}\nRun `orbit plugins list` to see available plugins.")
    };

    let mut state = PluginState::load();

    if !state.is_enabled(name) {
        println!("  {name} is not enabled.");
        return Ok(());
    }

    state.disable(name);
    state.save()?;

    if plugin.has_mcp() {
        let instances = PluginInstances::load();
        plugin::remove_plugin_mcps(&plugin)?;
        remove_instance_mcps(&plugin, &instances)?;

        let mut mcp_names: Vec<String> = plugin.mcp.iter().map(|m| m.name.clone()).collect();
        for r in instances.for_plugin(name) {
            mcp_names.push(format!("{}-{}", name, r.name));
        }
        if mcp_names.is_empty() {
            println!("  \x1b[32m✓\x1b[0m  {name} disabled.");
        } else {
            println!(
                "  \x1b[32m✓\x1b[0m  {name} disabled — MCP removed: {}",
                mcp_names.join(", ")
            );
        }
    } else {
        println!("  \x1b[32m✓\x1b[0m  {name} disabled.");
    }

    Ok(())
}

// ── auth ──────────────────────────────────────────────────────────────────────

fn auth(name: &str, list: bool, remove: Option<&str>) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}\nRun `orbit plugins list` to see available plugins.")
    };

    // Multi-instance plugin — delegate to dedicated flow.
    if plugin.instance.is_some() {
        return auth_multi_instance(&plugin, list, remove);
    }

    if list || remove.is_some() {
        bail!("{name} is not a multi-instance plugin — --list and --remove are not available.");
    }

    let Some(auth_spec) = &plugin.auth else {
        println!("  {name} has no auth configuration.");
        return Ok(());
    };

    println!("  Configuring auth for: \x1b[1m{}\x1b[0m", plugin.name);
    println!("  {}", auth_spec.hint);
    println!();

    // OAuth 2.1 PKCE flow — takes priority over static vars
    if let Some(oauth_spec) = &auth_spec.oauth {
        let already_set = orbit_core::secrets::keychain_get(&oauth_spec.token_key).is_ok();
        if already_set {
            print!("  Token already stored. Re-authorize? [y/N]: ");
            use std::io::Write;
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                println!("  Keeping existing token.");
                return Ok(());
            }
        }
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(super::oauth::run_oauth_flow(name, oauth_spec))
        })?;
        println!();
        println!("  \x1b[32m✓\x1b[0m  Auth configured for {}.", plugin.name);
        if plugin.has_mcp() && !PluginState::load().is_enabled(name) {
            println!("     Run `orbit plugins enable {name}` to activate the MCP server.");
        }
        return Ok(());
    }

    if auth_spec.vars.is_empty() && auth_spec.cmd.is_none() {
        println!("  No interactive setup available — follow the hint above.");
        return Ok(());
    }

    // ── collect and store credential vars ─────────────────────────────────────
    for var in &auth_spec.vars {
        let already_set = secrets::keychain_get(&var.name).is_ok();
        let default_hint = if already_set { "<already set>" } else { "" };

        let value = if var.secret {
            ask_secret(
                &format!(
                    "  {} (secret, leave blank to keep existing)",
                    var.description
                ),
                already_set,
            )?
        } else {
            ask(&format!("  {}", var.description), default_hint)?
        };

        if value.is_empty() {
            if already_set {
                println!("    \x1b[2m↩  {} unchanged\x1b[0m", var.name);
                continue;
            }
            if var.optional {
                println!("    \x1b[2m-  {} skipped (optional)\x1b[0m", var.name);
                continue;
            }
            bail!("{} is required and cannot be blank", var.name);
        }

        secrets::keychain_set(&var.name, &value)?;
        println!("    \x1b[32m✓\x1b[0m  {} stored in keychain", var.name);
    }

    // ── run optional CLI auth command ─────────────────────────────────────────
    if let Some(cmd) = &auth_spec.cmd {
        println!();
        println!("  Running: {cmd}");
        let mut command = Command::new("sh");
        command.arg("-c").arg(cmd);
        for var in &auth_spec.vars {
            if let Ok(value) = secrets::keychain_get(&var.name) {
                command.env(&var.name, value);
            }
        }
        let status = command.status()?;
        if !status.success() {
            bail!("auth command exited with failure");
        }
    }

    println!();
    println!("  \x1b[32m✓\x1b[0m  Auth configured for {}.", plugin.name);
    if plugin.has_mcp() && !PluginState::load().is_enabled(name) {
        println!("     Run `orbit plugins enable {name}` to activate the MCP server.");
    }

    Ok(())
}

fn auth_multi_instance(plugin: &Plugin, list: bool, remove: Option<&str>) -> Result<()> {
    let spec = plugin.instance.as_ref().expect("checked by caller");
    let name = &plugin.name;
    let mut instances = PluginInstances::load();

    // ── --list ────────────────────────────────────────────────────────────────
    if list {
        let configured: Vec<_> = instances.for_plugin(name).collect();
        if configured.is_empty() {
            println!("  No instances configured for {name}.");
            println!("  Run `orbit plugins auth {name}` to add one.");
        } else {
            println!("  Configured instances for \x1b[1m{name}\x1b[0m:");
            for r in configured {
                let mcp_key = format!("{name}-{}", r.name);
                let vars_display: Vec<String> =
                    r.vars.iter().map(|(k, v)| format!("{k}={v}")).collect();
                println!("    \x1b[32m●\x1b[0m  {}  (MCP: {})", r.name, mcp_key);
                if !vars_display.is_empty() {
                    println!("       {}", vars_display.join("  "));
                }
            }
        }
        return Ok(());
    }

    // ── --remove ──────────────────────────────────────────────────────────────
    if let Some(instance_name) = remove {
        if instances.remove(name, instance_name) {
            instances.save()?;
            // Regenerate MCP file if the plugin is currently enabled.
            if PluginState::load().is_enabled(name) {
                remove_instance_mcps(plugin, &instances)?;
                add_instance_mcps(plugin, &instances)?;
            }
            println!("  \x1b[32m✓\x1b[0m  Instance '{instance_name}' removed from {name}.");
        } else {
            println!("  Instance '{instance_name}' not found in {name}.");
        }
        return Ok(());
    }

    // ── add / update an instance ──────────────────────────────────────────────
    if let Some(auth_spec) = &plugin.auth {
        println!("  \x1b[1m{name}\x1b[0m — {}", auth_spec.hint);
        println!();
    }

    let instance_name = ask("  Instance name (e.g. prod, staging)", "")?;
    if instance_name.is_empty() {
        bail!("instance name cannot be blank");
    }
    if !instance_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        bail!("instance name must be alphanumeric (dashes and underscores allowed)");
    }

    println!();
    let mut non_secret_vars: HashMap<String, String> = HashMap::new();

    for var in &spec.vars {
        let keychain_key = instance_keychain_key(name, &instance_name, &var.name);
        let already_set = secrets::keychain_get(&keychain_key).is_ok();
        let default_hint = if already_set { "<already set>" } else { "" };

        let value = if var.secret {
            ask_secret(
                &format!(
                    "  {} (secret, leave blank to keep existing)",
                    var.description
                ),
                already_set,
            )?
        } else {
            ask(&format!("  {}", var.description), default_hint)?
        };

        if value.is_empty() {
            if already_set {
                println!("    \x1b[2m↩  {} unchanged\x1b[0m", var.name);
                if !var.secret {
                    // Preserve existing non-secret value.
                    if let Some(existing) = instances
                        .for_plugin(name)
                        .find(|r| r.name == instance_name)
                        .and_then(|r| r.vars.get(&var.name))
                    {
                        non_secret_vars.insert(var.name.clone(), existing.clone());
                    }
                }
                continue;
            }
            if var.optional {
                println!("    \x1b[2m-  {} skipped (optional)\x1b[0m", var.name);
                continue;
            }
            bail!("{} is required and cannot be blank", var.name);
        }

        if var.secret {
            secrets::keychain_set(&keychain_key, &value)?;
            println!("    \x1b[32m✓\x1b[0m  {} stored in keychain ({})", var.name, keychain_key);
        } else {
            non_secret_vars.insert(var.name.clone(), value);
            println!("    \x1b[32m✓\x1b[0m  {} stored", var.name);
        }
    }

    let record = PluginInstanceRecord {
        plugin: name.clone(),
        name: instance_name.clone(),
        vars: non_secret_vars,
    };
    instances.upsert(record);
    instances.save()?;

    // Regenerate MCP entries if plugin is currently enabled.
    if PluginState::load().is_enabled(name) {
        remove_instance_mcps(plugin, &instances)?;
        add_instance_mcps(plugin, &instances)?;
        println!();
        println!(
            "  \x1b[32m●\x1b[0m  Instance '{instance_name}' configured — MCP '{name}-{instance_name}' updated."
        );
    } else {
        println!();
        println!(
            "  \x1b[32m✓\x1b[0m  Instance '{instance_name}' configured."
        );
        println!("     Run `orbit plugins enable {name}` to activate the MCP server.");
    }

    Ok(())
}

fn ask_secret(prompt: &str, has_existing: bool) -> Result<String> {
    use crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{self, ClearType},
    };
    use std::io::stderr;

    print!("{prompt}: ");
    io::stdout().flush()?;

    terminal::enable_raw_mode()?;
    let mut value = String::new();

    loop {
        if let Event::Key(key) = event::read()? {
            match (key.modifiers, key.code) {
                // submit
                (_, KeyCode::Enter) => {
                    break;
                }
                // ctrl+c / ctrl+d → abort
                (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('d')) => {
                    terminal::disable_raw_mode()?;
                    println!();
                    bail!("cancelled");
                }
                // backspace
                (_, KeyCode::Backspace) => {
                    value.pop();
                }
                // printable char
                (_, KeyCode::Char(c)) => {
                    value.push(c);
                }
                _ => {}
            }
        }
    }

    terminal::disable_raw_mode()?;
    // print newline without echoing the secret
    let mut err = stderr();
    execute!(err, cursor::MoveToNextLine(1))?;
    let _ = execute!(err, terminal::Clear(ClearType::CurrentLine));
    println!();

    if value.is_empty() && has_existing {
        return Ok(String::new()); // caller interprets blank + has_existing as "keep"
    }
    Ok(value)
}

// ── info ──────────────────────────────────────────────────────────────────────

fn info(name: &str) -> Result<()> {
    let Some(plugin) = plugin::find(name) else {
        bail!("plugin not found: {name}\nRun `orbit plugins list` to see available plugins.")
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

    if let Some(spec) = &plugin.instance {
        let instances = PluginInstances::load();
        let configured: Vec<_> = instances.for_plugin(name).collect();
        println!();
        println!("  instances");
        if configured.is_empty() {
            println!("    none — run: orbit plugins auth {name}");
        } else {
            let var_w = spec.vars.iter().map(|v| v.name.len()).max().unwrap_or(4);
            for record in configured {
                println!("    \x1b[1m{}\x1b[0m", record.name);
                for var in &spec.vars {
                    if var.secret {
                        println!("      {:<var_w$}  [keychain]", var.name, var_w = var_w);
                    } else if let Some(val) = record.vars.get(&var.name) {
                        println!("      {:<var_w$}  {val}", var.name, var_w = var_w);
                    }
                }
            }
        }
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
            println!(
                "    \x1b[32m[active]\x1b[0m  {}",
                plugin::plugins_mcp_path().display()
            );
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

    if let Some(exec) = &plugin.executor {
        println!();
        println!("  executor");
        println!("    command   {}", exec.command.join(" "));
        if !exec.params.is_empty() {
            println!();
            let name_w = exec
                .params
                .iter()
                .map(|p| p.name.len())
                .max()
                .unwrap_or(4)
                .max(4);
            println!(
                "    {:<name_w$}  description                required  default",
                "param",
                name_w = name_w
            );
            println!("    {}", "-".repeat(name_w + 50));
            for p in &exec.params {
                let required = if p.required { "yes" } else { "no " };
                let default = p.default.as_deref().unwrap_or("—");
                println!(
                    "    {:<name_w$}  {:<27}  {required}       {default}",
                    p.name,
                    p.description,
                    name_w = name_w,
                );
            }
        }
        println!();
        println!("    usage: orbit plugins run {name} --param key=value");
    }

    println!();
    Ok(())
}

// ── run executor ──────────────────────────────────────────────────────────────

fn run_executor(name: &str, raw_params: &[String]) -> Result<()> {
    let params: HashMap<String, String> = raw_params
        .iter()
        .map(|s| {
            let (k, v) = s.split_once('=').unwrap_or((s, ""));
            (k.to_string(), v.to_string())
        })
        .collect();

    let rendered_cmd = if name == "shell" {
        let command = params
            .get("command")
            .ok_or_else(|| anyhow::anyhow!("shell executor requires --param command=<cmd>"))?;
        vec!["sh".to_string(), "-c".to_string(), command.clone()]
    } else {
        let plugin = plugin::find(name).ok_or_else(|| {
            anyhow::anyhow!(
                "plugin '{name}' not found — run `orbit plugins list` to see available plugins"
            )
        })?;
        plugin.render_executor_command(&params)?
    };

    println!("  \x1b[2m$ {}\x1b[0m", rendered_cmd.join(" "));
    println!();

    let status = Command::new(&rendered_cmd[0])
        .args(&rendered_cmd[1..])
        .status()?;

    if !status.success() {
        bail!("command exited with status {}", status.code().unwrap_or(-1));
    }

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

    if !supported.is_empty() && !supported.contains(&default) {
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

    println!("plugins");
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

    let state = PluginState::load();

    println!("plugins\n");
    println!(
        "  \x1b[2mPlugins extend orbit with tools, MCP servers, and session context.\x1b[0m\n"
    );

    let name_w = plugins.iter().map(|p| p.name.len()).max().unwrap_or(8).max(8);
    let desc_w: usize = 50;
    let sep_w = 5 + name_w + 2 + desc_w + 2 + 16;

    println!(
        "     \x1b[2m{:<name_w$}  {:<desc_w$}  tags\x1b[0m",
        "name",
        "description",
        name_w = name_w,
        desc_w = desc_w,
    );
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));

    for p in &plugins {
        let installed = p.is_installed();
        let enabled = state.is_enabled(&p.name);

        let status = match (installed, enabled && p.has_mcp()) {
            (_, true) => "\x1b[32m●\x1b[0m",
            (true, _) => "\x1b[32m✓\x1b[0m",
            _ => "\x1b[2m○\x1b[0m",
        };
        let desc = truncate_desc(&p.description, desc_w);
        let mcp_tag = if p.has_mcp() {
            if enabled { "\x1b[32mmcp ●\x1b[0m" } else { "\x1b[2mmcp ○\x1b[0m" }
        } else {
            ""
        };

        println!(
            "  {status}  {:<name_w$}  {desc:<desc_w$}  {mcp_tag}",
            p.name,
            name_w = name_w,
            desc_w = desc_w,
        );
    }

    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));
    println!(
        "  \x1b[2m● installed+MCP active  ✓ installed  ○ not installed  ·  mcp ● active  mcp ○ inactive\x1b[0m"
    );

    let installed_count = plugins.iter().filter(|p| p.is_installed()).count();
    println!();
    println!(
        "  {installed_count}/{total} installed  ·  orbit plugins install/enable <name>",
        total = plugins.len()
    );

    // Prompt to install uninstalled plugins
    if !yes {
        println!();
        for p in &uninstalled {
            let should_install = confirm(&format!("Install {}?", p.name), false)?;
            if should_install && let Some(m) = p.best_install_method() {
                println!("  Installing {}...", p.name);
                let status = Command::new(&m.cmd[0]).args(&m.cmd[1..]).status();
                match status {
                    Ok(s) if s.success() => {
                        println!("  \x1b[32m✓\x1b[0m  installed");
                        if p.has_mcp() {
                            println!(
                                "     \x1b[2mrun `orbit plugins enable {}` to activate MCP servers\x1b[0m",
                                p.name
                            );
                        }
                    }
                    _ => println!(
                        "  \x1b[31m✗\x1b[0m  failed — run: {}",
                        m.cmd.join(" ")
                    ),
                }
            }
        }
    }

    Ok(())
}

fn ask(prompt: &str, default: &str) -> Result<String> {
    print!("{prompt} [{default}]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    Ok(if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    })
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
