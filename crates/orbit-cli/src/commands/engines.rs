use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use orbit_core::catalog::{self, EngineEntry};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    time::Duration,
};

use super::auth::{detect_auth, AuthStatus};

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct EnginesArgs {
    #[command(subcommand)]
    pub command: Option<EnginesCommand>,
}

#[derive(Debug, Subcommand)]
pub enum EnginesCommand {
    /// List all catalog engines with install status and version
    List,
    /// Install an engine from the catalog
    Install {
        /// Engine name
        name: String,
        /// Accept without confirming
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Update one or all installed engines to the latest version
    Update {
        /// Engine name (omit to update all installed engines)
        name: Option<String>,
    },
    /// Show detailed info for an engine
    Info {
        /// Engine name
        name: String,
    },
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run(args: EnginesArgs) -> Result<()> {
    match args.command.unwrap_or(EnginesCommand::List) {
        EnginesCommand::List => cmd_list(),
        EnginesCommand::Install { name, yes } => cmd_install(&name, yes),
        EnginesCommand::Update { name } => cmd_update(name.as_deref()),
        EnginesCommand::Info { name } => cmd_info(&name),
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

fn cmd_list() -> Result<()> {
    let engines = catalog::engines();
    println!("engines\n");

    let name_w = engines.iter().map(|e| e.name.len()).max().unwrap_or(8).max(8);

    for engine in &engines {
        let installed = bin_available(&engine.bin);

        if installed {
            let installed_ver = installed_version(&engine.bin).unwrap_or_else(|| "?".to_string());
            let cached_latest = if engine.npm_package.is_empty() {
                None
            } else {
                cached_npm_version(&engine.npm_package)
            };

            let update_tag = match &cached_latest {
                Some(latest) if latest != &installed_ver && !installed_ver.starts_with('?') => {
                    format!("  \x1b[33m→ {latest} available\x1b[0m")
                }
                Some(_) => "  \x1b[32mup to date\x1b[0m".to_string(),
                None if engine.npm_package.is_empty() => String::new(),
                None => "  \x1b[2mrun orbit engines info for latest\x1b[0m".to_string(),
            };

            println!(
                "  \x1b[32m✓\x1b[0m  {name:<name_w$}  \x1b[2mv{installed_ver}\x1b[0m{update_tag}",
                name = engine.name,
                name_w = name_w,
            );
        } else {
            println!(
                "  \x1b[2m○\x1b[0m  {name:<name_w$}  not installed  \x1b[2mnpm install -g {}\x1b[0m",
                engine.npm_package,
                name = engine.name,
                name_w = name_w,
            );
        }
    }

    println!();
    let installed_count = engines.iter().filter(|e| bin_available(&e.bin)).count();
    println!("  {installed_count}/{total} installed  ·  orbit engines install/update <name>", total = engines.len());

    Ok(())
}

// ── install ───────────────────────────────────────────────────────────────────

fn cmd_install(name: &str, yes: bool) -> Result<()> {
    let engine = catalog::engine_by_name(name).ok_or_else(|| engine_not_found(name))?;

    if bin_available(&engine.bin) {
        // For extension-based engines (e.g. copilot via gh), bin existing
        // doesn't guarantee the extension is installed — just proceed.
        if engine.install_cmd.is_empty() {
            let ver = installed_version(&engine.bin).unwrap_or_else(|| "?".to_string());
            println!("  \x1b[32m✓\x1b[0m  {} is already installed (v{ver})", engine.name);
            return Ok(());
        }
    }

    // Validate prerequisites
    if !engine.install_cmd.is_empty() {
        // Custom install — verify the prerequisite binary exists
        let prereq = &engine.install_cmd[0];
        if !bin_available(prereq) {
            bail!(
                "`{prereq}` is not available — install it first before running `orbit engines install {name}`"
            );
        }
    } else {
        if !bin_available("npm") {
            bail!("npm is not available — install Node.js first: https://nodejs.org");
        }
        if engine.npm_package.is_empty() {
            bail!("no install command defined for {name}");
        }
    }

    if !yes && !confirm(&format!("Install {}?", engine.name))? {
        println!("  Skipped.");
        return Ok(());
    }

    print!("  Installing {}...", engine.name);
    io::stdout().flush()?;

    let status = if !engine.install_cmd.is_empty() {
        let (cmd, args) = engine.install_cmd.split_first().unwrap();
        Command::new(cmd).args(args).status()?
    } else {
        Command::new("npm")
            .args(["install", "-g", &engine.npm_package])
            .status()?
    };

    if status.success() {
        let ver = installed_version(&engine.bin).unwrap_or_else(|| "?".to_string());
        println!(" \x1b[32mdone\x1b[0m  v{ver}");
        println!();
        print_auth_hint(&engine);
    } else {
        println!(" \x1b[31mfailed\x1b[0m");
        let cmd_str = if !engine.install_cmd.is_empty() {
            engine.install_cmd.join(" ")
        } else {
            format!("npm install -g {}", engine.npm_package)
        };
        bail!("{cmd_str} failed");
    }

    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

fn cmd_update(name: Option<&str>) -> Result<()> {
    let engines = catalog::engines();

    let targets: Vec<&EngineEntry> = if let Some(n) = name {
        let e = engines.iter().find(|e| e.name == n);
        match e {
            None => bail!("{}", engine_not_found(n)),
            Some(e) => vec![e],
        }
    } else {
        engines.iter().filter(|e| bin_available(&e.bin)).collect()
    };

    if targets.is_empty() {
        println!("  No engines installed — run `orbit engines install <name>` first.");
        return Ok(());
    }

    // Verify npm is available for any npm-based engine in the target set
    let needs_npm = targets.iter().any(|e| e.update_cmd.is_empty() && !e.npm_package.is_empty());
    if needs_npm && !bin_available("npm") {
        bail!("npm is not available — install Node.js first: https://nodejs.org");
    }

    for engine in &targets {
        let before = installed_version(&engine.bin).unwrap_or_else(|| "?".to_string());
        print!("  Updating {}...", engine.name);
        io::stdout().flush()?;

        let status = if !engine.update_cmd.is_empty() {
            let (cmd, args) = engine.update_cmd.split_first().unwrap();
            Command::new(cmd)
                .args(args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?
        } else if !engine.npm_package.is_empty() {
            Command::new("npm")
                .args(["install", "-g", &format!("{}@latest", engine.npm_package)])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?
        } else {
            println!(" \x1b[33mskipped\x1b[0m  no update command defined");
            continue;
        };

        if status.success() {
            let after = installed_version(&engine.bin).unwrap_or_else(|| "?".to_string());
            if before == after {
                println!(" \x1b[32mup to date\x1b[0m  v{after}");
            } else {
                println!(" \x1b[32mdone\x1b[0m  v{before} → v{after}");
            }
            // Refresh cache with new version
            save_npm_version_cache(&engine.npm_package, &after);
        } else {
            println!(" \x1b[31mfailed\x1b[0m");
        }
    }

    Ok(())
}

// ── info ──────────────────────────────────────────────────────────────────────

fn cmd_info(name: &str) -> Result<()> {
    let engine = catalog::engine_by_name(name).ok_or_else(|| engine_not_found(name))?;

    let installed = bin_available(&engine.bin);
    let installed_ver = if installed {
        installed_version(&engine.bin).unwrap_or_else(|| "?".to_string())
    } else {
        String::new()
    };

    // Fetch latest from npm (with timeout) and cache — only for npm-based engines
    let latest_ver = if engine.npm_package.is_empty() {
        None
    } else {
        fetch_npm_version(&engine.npm_package)
    };

    println!("\x1b[1m{}\x1b[0m", engine.name);
    println!("  {}", engine.description);
    println!();

    let info_w = 14usize;
    if !engine.npm_package.is_empty() {
        info_row("package", info_w, &engine.npm_package);
    }
    if !engine.install_cmd.is_empty() {
        info_row("install via", info_w, &engine.install_cmd.join(" "));
    }

    if installed {
        let ver_detail = match &latest_ver {
            Some(l) if l != &installed_ver => {
                format!("v{installed_ver}  \x1b[33m→ v{l} available\x1b[0m  run: orbit engines update {}", engine.name)
            }
            Some(_) => format!("v{installed_ver}  \x1b[32mup to date\x1b[0m"),
            None => format!("v{installed_ver}"),
        };
        info_row("installed", info_w, &ver_detail);
    } else {
        info_row("installed", info_w, "\x1b[31mnot installed\x1b[0m");
        info_row("install", info_w, &format!("npm install -g {}", engine.npm_package));
    }

    // Auth status
    let auth_str = match detect_auth(&engine) {
        AuthStatus::Configured(s) => format!("\x1b[32mconfigured\x1b[0m  {s}"),
        AuthStatus::NotConfigured => {
            format!("\x1b[33mnot configured\x1b[0m  orbit auth {}", engine.name)
        }
    };
    info_row("auth", info_w, &auth_str);
    info_row("auth hint", info_w, &engine.auth_hint);

    Ok(())
}

fn info_row(label: &str, label_w: usize, rest: &str) {
    let pad = " ".repeat(label_w.saturating_sub(label.len()));
    println!("  \x1b[2m{label}\x1b[0m{pad}  {rest}");
}

// ── version helpers ───────────────────────────────────────────────────────────

/// Run `<bin> --version` and return the version string (without leading `v`).
/// Takes only the first token that looks like a semver number.
fn installed_version(bin: &str) -> Option<String> {
    let out = Command::new(bin).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    // Find first whitespace-delimited token that contains a digit and a dot
    for token in line.split_whitespace() {
        let stripped = token.trim_start_matches('v');
        if stripped.contains('.') && stripped.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(stripped.to_string());
        }
    }
    None
}

/// Return cached npm version if available (no network call).
fn cached_npm_version(package: &str) -> Option<String> {
    let path = npm_cache_path(package);
    let Ok(meta) = fs::metadata(&path) else { return None };
    let Ok(modified) = meta.modified() else { return None };
    // Return cached value regardless of age (for list command — fast path)
    if modified.elapsed().is_ok() {
        let text = fs::read_to_string(&path).ok()?;
        let v = text.trim().to_string();
        if !v.is_empty() { Some(v) } else { None }
    } else {
        None
    }
}

/// Fetch npm latest version with a 5-second timeout. Cache the result.
fn fetch_npm_version(package: &str) -> Option<String> {
    // Check cache first
    let path = npm_cache_path(package);
    if let Ok(meta) = fs::metadata(&path)
        && let Ok(modified) = meta.modified()
    {
        let age = modified.elapsed().unwrap_or(Duration::MAX);
        if age < Duration::from_secs(86_400)
            && let Ok(text) = fs::read_to_string(&path)
        {
            let v = text.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }

    if !bin_available("npm") {
        return None;
    }

    // Run npm view with a timeout via a child process (best-effort; npm is slow)
    let out = Command::new("npm")
        .args(["view", package, "version", "--json"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&out.stdout);
    // npm view returns `"1.2.3"\n` (with quotes) when --json is used
    let version = text.trim().trim_matches('"').to_string();
    if version.is_empty() || !version.contains('.') {
        return None;
    }

    save_npm_version_cache(package, &version);
    Some(version)
}

fn save_npm_version_cache(package: &str, version: &str) {
    let path = npm_cache_path(package);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, version.as_bytes());
}

fn npm_cache_path(package: &str) -> PathBuf {
    let safe_name = package.replace(['/', '@'], "_");
    data_dir().join("engine-versions").join(safe_name)
}

fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("orbit")
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share/orbit"))
            .unwrap_or_else(|| PathBuf::from("/tmp/orbit"))
    }
}

// ── misc helpers ──────────────────────────────────────────────────────────────

fn print_auth_hint(engine: &EngineEntry) {
    match detect_auth(engine) {
        AuthStatus::Configured(s) => {
            println!("  \x1b[32m✓ auth\x1b[0m  {s}");
        }
        AuthStatus::NotConfigured => {
            println!("  \x1b[33m○ auth\x1b[0m  not configured");
            println!("  \x1b[2m{}\x1b[0m", engine.auth_hint);
            println!("  Run: orbit auth {}", engine.name);
        }
    }
}

fn engine_not_found(name: &str) -> anyhow::Error {
    let names: Vec<String> = catalog::engines().into_iter().map(|e| e.name).collect();
    anyhow::anyhow!(
        "unknown engine: {name}\n  Available: {}",
        names.join(", ")
    )
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

fn confirm(prompt: &str) -> Result<bool> {
    print!("  {prompt} [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}
