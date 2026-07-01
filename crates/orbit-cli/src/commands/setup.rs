use anyhow::Result;
use clap::Args;
use orbit_core::{
    catalog::{self, McpEntry},
    user_config::UserConfig,
};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use super::plugins::setup_plugins;

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// AI workspace root directory [default: ~/AI]
    #[arg(long)]
    pub ai_root: Option<PathBuf>,

    /// Default AI engine to use [default: opencode]
    #[arg(long)]
    pub default_engine: Option<String>,

    /// Default tenant (leave empty to always specify it explicitly)
    #[arg(long)]
    pub default_tenant: Option<String>,

    /// Default workspace name (leave empty to always specify it explicitly)
    #[arg(long)]
    pub default_workspace: Option<String>,

    /// Directory where orbit binary is installed [default: ~/.local/bin]
    #[arg(long)]
    pub install_dir: Option<PathBuf>,

    /// Accept all defaults without prompting
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Print what would be done without writing anything
    #[arg(long)]
    pub dry_run: bool,

    /// Skip engine installation prompts
    #[arg(long)]
    pub no_install: bool,

    /// Skip plugin installation prompts
    #[arg(long)]
    pub no_plugins: bool,

    /// Skip MCP configuration prompts
    #[arg(long)]
    pub no_mcps: bool,
}

pub async fn run(args: SetupArgs) -> Result<()> {
    println!();
    println!("  Welcome to Orbit — AI ecosystem CLI");
    println!();

    let current = UserConfig::load();
    let engines = catalog::engines();
    let engine_names: Vec<&str> = engines.iter().map(|e| e.name.as_str()).collect();
    let engine_names_str = engine_names.join(" / ");

    // ── collect values (flags → interactive → default) ────────────────────────
    let ai_root = match args.ai_root {
        Some(p) => p,
        None => {
            let default = current.workspace.ai_root.to_string_lossy().into_owned();
            if args.yes {
                current.workspace.ai_root.clone()
            } else {
                ask("AI workspace root", &default)?.into()
            }
        }
    };

    let default_engine = match args.default_engine {
        Some(e) => e,
        None => {
            let default = &current.engine.default;
            if args.yes {
                default.clone()
            } else {
                ask(&format!("Default engine ({})", engine_names_str), default)?
            }
        }
    };

    let default_tenant = match args.default_tenant {
        Some(t) => t,
        None => {
            let default = if current.engine.default_tenant.is_empty() {
                "(none)"
            } else {
                &current.engine.default_tenant
            };
            if args.yes {
                if default == "(none)" {
                    String::new()
                } else {
                    default.to_string()
                }
            } else {
                let val = ask("Default tenant (leave blank to skip)", default)?;
                if val == "(none)" { String::new() } else { val }
            }
        }
    };

    let default_workspace = match args.default_workspace {
        Some(w) => w,
        None => {
            let default = if current.engine.default_workspace.is_empty() {
                "(none)"
            } else {
                &current.engine.default_workspace
            };
            if args.yes {
                if default == "(none)" {
                    String::new()
                } else {
                    default.to_string()
                }
            } else {
                let val = ask("Default workspace name (leave blank to skip)", default)?;
                if val == "(none)" { String::new() } else { val }
            }
        }
    };

    let install_dir = match args.install_dir {
        Some(d) => d,
        None => {
            let default = current.install.dir.to_string_lossy().into_owned();
            if args.yes {
                current.install.dir.clone()
            } else {
                ask("Install directory", &default)?.into()
            }
        }
    };

    // ── build final config ────────────────────────────────────────────────────
    let mut cfg = UserConfig::default();
    cfg.workspace.ai_root = ai_root.clone();
    cfg.engine.default = default_engine.clone();
    cfg.engine.default_tenant = default_tenant.clone();
    cfg.engine.default_workspace = default_workspace.clone();
    cfg.install.dir = install_dir.clone();

    if args.dry_run {
        println!();
        println!("  [dry-run] would write {}:", UserConfig::path().display());
        println!("{}", toml::to_string_pretty(&cfg)?);
        return Ok(());
    }

    // ── save config ───────────────────────────────────────────────────────────
    cfg.save()?;
    println!();
    println!("  Config saved → {}", UserConfig::path().display());

    // ── self-install binary ───────────────────────────────────────────────────
    let install_dir_expanded = orbit_core::user_config::expand_tilde(&install_dir);
    let current_exe = std::env::current_exe()?;
    let target = install_dir_expanded.join("orbit");

    if current_exe == target {
        println!("  Binary already at {} — skipping copy", target.display());
    } else {
        fs::create_dir_all(&install_dir_expanded)?;
        fs::copy(&current_exe, &target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        }
        println!("  Binary installed → {}", target.display());
    }

    // ── PATH hint ─────────────────────────────────────────────────────────────
    let install_dir_str = install_dir_expanded.to_string_lossy();
    let path_env = std::env::var("PATH").unwrap_or_default();
    if !path_env.split(':').any(|p| p == install_dir_str.as_ref()) {
        println!();
        println!("  Add to your shell profile:");
        println!("    export PATH=\"{install_dir_str}:$PATH\"");
    }

    // ── engine install & auth ─────────────────────────────────────────────────
    if !args.no_install {
        println!();
        setup_engines(&default_engine, args.yes).await?;
    }

    // ── plugin install ────────────────────────────────────────────────────────
    if !args.no_plugins && !args.yes {
        println!();
        setup_plugins(args.yes)?;
    }

    // ── MCP configuration ─────────────────────────────────────────────────────
    if !args.no_mcps && !args.yes {
        println!();
        setup_mcps()?;
    }

    // ── next steps ────────────────────────────────────────────────────────────
    println!();
    if !ai_root.exists() {
        println!("  AI root does not exist yet. To clone a governance repo:");
        println!("    orbit init <governance-url>");
    } else {
        println!("  Ready. Run `orbit launch` to start a session.");
    }
    println!();

    Ok(())
}

// ── engine setup ─────────────────────────────────────────────────────────────

async fn setup_engines(default_engine: &str, yes: bool) -> Result<()> {
    println!("  Checking engines...");
    println!();

    let engines = catalog::engines();
    let has_npm = bin_available("npm");

    for engine in &engines {
        let installed = bin_available(&engine.bin);

        if installed {
            println!("  \x1b[32m✓\x1b[0m  {}", engine.name);
        } else {
            println!("  \x1b[33m○\x1b[0m  {} — not installed", engine.name);

            if !has_npm {
                println!("      install Node.js first: https://nodejs.org");
            } else {
                let should_install = yes || engine.name == default_engine
                    || confirm(&format!("    Install {}?", engine.name), false)?;

                if should_install {
                    print!("    Installing {}...", engine.name);
                    io::stdout().flush()?;
                    let install_cmd: Vec<&str> = {
                        let mut v = vec!["npm", "install", "-g"];
                        v.push(engine.npm_package.as_str());
                        v
                    };
                    let status = Command::new(install_cmd[0])
                        .args(&install_cmd[1..])
                        .status();
                    match status {
                        Ok(s) if s.success() => println!(" done"),
                        _ => println!(
                            " \x1b[31mfailed\x1b[0m — run manually: npm install -g {}",
                            engine.npm_package
                        ),
                    }
                }
            }
        }

        println!("      \x1b[2mauth: {}\x1b[0m", engine.auth_hint);
    }

    Ok(())
}

// ── MCP setup ─────────────────────────────────────────────────────────────────

fn setup_mcps() -> Result<()> {
    let mcps = catalog::mcps();

    println!("  Available MCPs:");
    println!();
    for (i, mcp) in mcps.iter().enumerate() {
        println!("  {}. {}  —  {}", i + 1, mcp.name, mcp.description);
    }
    println!();

    if !confirm("  Configure any MCPs now?", false)? {
        println!("  Skipped. Use `orbit mcp <name> enable` to configure later.");
        return Ok(());
    }

    let mut selected: Vec<&McpEntry> = Vec::new();
    for mcp in &mcps {
        if confirm(&format!("    Enable {}?", mcp.name), false)? {
            selected.push(mcp);
        }
    }

    if selected.is_empty() {
        println!("  No MCPs selected.");
        return Ok(());
    }

    // Collect vars and build mcp.json entries
    let mut mcp_config: HashMap<String, serde_json::Value> = HashMap::new();

    for mcp in &selected {
        let mut env: HashMap<String, String> = HashMap::new();

        for var in &mcp.required_vars {
            let default = var.default.as_deref().unwrap_or("");
            let prompt = if var.secret {
                format!("    {} (secret)", var.name)
            } else {
                var.name.clone()
            };
            let val = ask(&prompt, default)?;
            if !val.is_empty() {
                env.insert(var.name.clone(), val);
            }
        }

        for var in &mcp.optional_vars {
            let default = var.default.as_deref().unwrap_or("");
            let val = ask(&format!("    {} (optional)", var.name), default)?;
            if !val.is_empty() && val != default {
                env.insert(var.name.clone(), val);
            }
        }

        let (command, args) = mcp.command.split_first().unwrap_or((&mcp.name, &[]));
        let mut entry = serde_json::json!({
            "command": command,
            "args": args,
        });
        if !env.is_empty() {
            entry["env"] = serde_json::to_value(&env)?;
        }
        mcp_config.insert(mcp.name.clone(), entry);
    }

    // Write to ~/.config/orbit/mcps.json
    let config_dir = orbit_core::user_config::UserConfig::path()
        .parent()
        .unwrap()
        .to_path_buf();
    let mcps_path = config_dir.join("mcps.json");
    let existing: HashMap<String, serde_json::Value> = if mcps_path.exists() {
        serde_json::from_str(&fs::read_to_string(&mcps_path)?).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let merged: serde_json::Map<String, serde_json::Value> =
        existing.into_iter().chain(mcp_config).collect();

    fs::write(&mcps_path, serde_json::to_string_pretty(&merged)?)?;
    println!();
    println!("  MCPs saved → {}", mcps_path.display());

    Ok(())
}

// ── helpers ────────────────────────────────────────────────────────────────────

fn bin_available(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn ask(question: &str, default: &str) -> Result<String> {
    print!("  {question} [{default}]: ");
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
