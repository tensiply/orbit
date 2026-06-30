use anyhow::Result;
use clap::Args;
use orbit_core::user_config::UserConfig;
use std::{
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
}

pub async fn run(args: SetupArgs) -> Result<()> {
    println!();
    println!("  Welcome to Orbit — AI ecosystem CLI");
    println!();

    let current = UserConfig::load();

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
                ask("Default engine (opencode / gemini / claude)", default)?
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

    let engines = [
        EngineInfo {
            name: "opencode",
            install_cmd: &["npm", "install", "-g", "opencode-ai"],
            auth_cmd: &["opencode", "auth"],
            auth_hint: "Run `opencode auth` or set OPENAI_API_KEY / provider keys",
        },
        EngineInfo {
            name: "gemini",
            install_cmd: &["npm", "install", "-g", "@google/gemini-cli"],
            auth_cmd: &["gemini", "auth"],
            auth_hint: "Run `gemini auth` or set GOOGLE_API_KEY / GEMINI_API_KEY",
        },
        EngineInfo {
            name: "claude",
            install_cmd: &["npm", "install", "-g", "@anthropic-ai/claude-code"],
            auth_cmd: &["claude", "auth", "login"],
            auth_hint: "Run `claude auth login` or set ANTHROPIC_API_KEY",
        },
    ];

    let has_npm = bin_available("npm");

    for engine in &engines {
        let installed = bin_available(engine.name);

        if installed {
            println!("  \x1b[32m✓\x1b[0m  {}", engine.name);
        } else {
            println!("  \x1b[33m○\x1b[0m  {} — not installed", engine.name);

            if !has_npm {
                println!("      install Node.js first: https://nodejs.org");
                continue;
            }

            let should_install = if yes || engine.name == default_engine {
                true
            } else {
                confirm(&format!("    Install {}?", engine.name), false)?
            };

            if should_install {
                print!("    Installing {}...", engine.name);
                io::stdout().flush()?;
                let status = Command::new(engine.install_cmd[0])
                    .args(&engine.install_cmd[1..])
                    .status();
                match status {
                    Ok(s) if s.success() => println!(" done"),
                    _ => println!(" \x1b[31mfailed\x1b[0m — run manually: {}", engine.install_cmd.join(" ")),
                }
            }
        }

        // Auth hint (always shown — we can't reliably detect auth state)
        println!("      \x1b[2mauth: {}\x1b[0m", engine.auth_hint);
    }

    Ok(())
}

struct EngineInfo {
    name: &'static str,
    install_cmd: &'static [&'static str],
    #[allow(dead_code)]
    auth_cmd: &'static [&'static str],
    auth_hint: &'static str,
}

fn bin_available(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── prompt helpers ────────────────────────────────────────────────────────────

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
