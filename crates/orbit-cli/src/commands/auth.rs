use anyhow::{Result, bail};
use clap::Args;
use orbit_core::{catalog, catalog::EngineEntry, resolver};
use orbit_engine::launcher::runtime;
use std::{fs, process::Command};

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

    let mut cmd = Command::new(parts[0]);
    cmd.args(&parts[1..]);

    // Auth is always workspace-level — one account per workspace regardless of
    // which tenant or project the user is currently in.
    match resolver::resolve_from_cwd() {
        Ok(scope) => {
            let workspace_runtime = runtime::workspace_runtime_dir_for_slug(&scope, &engine.name);
            let xdg_config = workspace_runtime.join("config");
            let xdg_data = workspace_runtime.join("data");
            fs::create_dir_all(&xdg_config)?;
            fs::create_dir_all(&xdg_data)?;

            cmd.env("XDG_CONFIG_HOME", &xdg_config);
            cmd.env("XDG_DATA_HOME", &xdg_data);

            // Gemini CLI reads GEMINI_CLI_HOME directly instead of XDG.
            if engine.name == "gemini" {
                cmd.env("GEMINI_CLI_HOME", &workspace_runtime);
            }

            let workspace = scope
                .workspace_root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());
            println!("workspace: {workspace}");
            println!("auth dir:  {}", xdg_data.display());
        }
        Err(_) => {
            println!("  note: not inside a workspace — auth will be stored globally");
        }
    }

    println!("Running: {}", engine.auth_cmd);
    println!();

    let status = cmd.status()?;

    if !status.success() {
        bail!("{} auth exited with non-zero status", engine.auth_cmd);
    }

    // After successful auth, resolve the GitHub username from the stored token
    // and cache it so dry-run can display it without a network call.
    if let Ok(scope) = resolver::resolve_from_cwd() {
        let workspace_runtime = runtime::workspace_runtime_dir_for_slug(&scope, &engine.name);
        let auth_file = workspace_runtime.join("data").join("opencode").join("auth.json");
        if auth_file.exists()
            && let Some(username) = resolve_github_username(&auth_file)
        {
            let account_file = workspace_runtime.join("data").join("opencode").join("account.json");
            let _ = fs::write(
                &account_file,
                format!("{{\"username\":\"{username}\"}}"),
            );
            println!("account:   {username}");
        }
    }

    Ok(())
}

/// Read the access token from opencode's auth.json and resolve the GitHub username.
fn resolve_github_username(auth_file: &std::path::Path) -> Option<String> {
    let content = fs::read_to_string(auth_file).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let token = v
        .as_object()?
        .values()
        .next()?
        .get("access")?
        .as_str()?
        .to_string();

    let output = Command::new("curl")
        .args(["-sf", "-H", &format!("Authorization: token {token}"), "https://api.github.com/user"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let resp: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    resp.get("login")?.as_str().map(|s| s.to_string())
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
