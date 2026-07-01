use anyhow::{Result, bail};
use clap::Args;
use orbit_core::{catalog, catalog::EngineEntry};
use std::process::Command;

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct AuthArgs {
    /// Engine name (omit to show all engines)
    pub engine: Option<String>,

    /// Only verify status — exit code 1 if any engine is not configured
    #[arg(long, short = 'c')]
    pub check: bool,
}

// ── auth status ───────────────────────────────────────────────────────────────

pub enum AuthStatus {
    /// Found a non-empty signal (env var or config dir). Contains a short label.
    Configured(String),
    NotConfigured,
}

/// Detect auth status for an engine by checking env vars and config dirs.
/// This is heuristic — it does not make any network calls.
pub fn detect_auth(engine: &EngineEntry) -> AuthStatus {
    let home = home_dir();

    for var in &engine.auth_env_vars {
        if let Ok(val) = std::env::var(var)
            && !val.trim().is_empty()
        {
            return AuthStatus::Configured(format!("${var}"));
        }
    }

    for dir in &engine.auth_config_dirs {
        let path = home.join(dir);
        if path.exists() {
            return AuthStatus::Configured(format!("~/{dir}"));
        }
    }

    AuthStatus::NotConfigured
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run(args: AuthArgs) -> Result<()> {
    match args.engine.as_deref() {
        None => cmd_status_all(args.check),
        Some(name) => {
            if args.check {
                cmd_check_one(name)
            } else {
                cmd_run_auth(name)
            }
        }
    }
}

// ── status all ────────────────────────────────────────────────────────────────

fn cmd_status_all(check_mode: bool) -> Result<()> {
    let engines = catalog::engines();

    if !check_mode {
        println!("auth status\n");
    }

    let name_w = engines
        .iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let mut any_missing = false;

    for engine in &engines {
        let installed = bin_available(&engine.bin);
        let status = detect_auth(engine);

        match (&status, installed) {
            (AuthStatus::Configured(signal), _) => {
                if !check_mode {
                    println!(
                        "  \x1b[32m✓\x1b[0m  {name:<name_w$}  {signal}",
                        name = engine.name,
                        name_w = name_w,
                    );
                }
            }
            (AuthStatus::NotConfigured, true) => {
                any_missing = true;
                if !check_mode {
                    println!(
                        "  \x1b[33m○\x1b[0m  {name:<name_w$}  not configured — run: orbit auth {name}",
                        name = engine.name,
                        name_w = name_w,
                    );
                }
            }
            (AuthStatus::NotConfigured, false) => {
                any_missing = true;
                if !check_mode {
                    println!(
                        "  \x1b[2m○\x1b[0m  {name:<name_w$}  not installed",
                        name = engine.name,
                        name_w = name_w,
                    );
                }
            }
        }
    }

    if !check_mode {
        println!();
        println!("  \x1b[2morbit auth <engine>  to start the engine's auth flow\x1b[0m");
    } else if any_missing {
        std::process::exit(1);
    }

    Ok(())
}

// ── check one engine ──────────────────────────────────────────────────────────

fn cmd_check_one(name: &str) -> Result<()> {
    let engine = catalog::engine_by_name(name).ok_or_else(|| engine_not_found_err(name))?;

    match detect_auth(&engine) {
        AuthStatus::Configured(_) => Ok(()),
        AuthStatus::NotConfigured => std::process::exit(1),
    }
}

// ── run native auth flow ──────────────────────────────────────────────────────

fn cmd_run_auth(name: &str) -> Result<()> {
    let engine = catalog::engine_by_name(name).ok_or_else(|| engine_not_found_err(name))?;

    if !bin_available(&engine.bin) {
        bail!(
            "{name} is not installed.\n  Install: npm install -g {}",
            engine.npm_package
        );
    }

    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        bail!(
            "orbit auth {name} requires an interactive terminal.\n  In CI or scripts, use: orbit auth --check"
        );
    }

    // auth_cmd examples: "opencode auth", "gemini auth", "claude auth login"
    let parts: Vec<&str> = engine.auth_cmd.split_whitespace().collect();
    if parts.is_empty() {
        bail!("no auth command configured for {name}");
    }

    println!("Running: {}", engine.auth_cmd);
    println!();

    let status = Command::new(parts[0]).args(&parts[1..]).status()?;

    if !status.success() {
        bail!("{} auth exited with non-zero status", engine.auth_cmd);
    }

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn engine_not_found_err(name: &str) -> anyhow::Error {
    let names: Vec<String> = catalog::engines().into_iter().map(|e| e.name).collect();
    anyhow::anyhow!("unknown engine: {name}\n  Available: {}", names.join(", "))
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

fn home_dir() -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
}
