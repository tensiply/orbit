use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use orbit_core::{channel::Channel, user_config::UserConfig};
use std::{fs, io::Write, path::PathBuf};

use crate::{commands::update, update_check};

#[derive(Debug, Args)]
pub struct ModeArgs {
    #[command(subcommand)]
    pub command: ModeCommand,
}

#[derive(Debug, Subcommand)]
pub enum ModeCommand {
    /// Show current mode and binary details
    Status,
    /// Switch to stable (download latest release from GitHub)
    Stable,
    /// Switch to dev (symlink to a local build)
    Dev {
        /// Path to the local orbit binary (e.g. ./target/release/orbit)
        path: Option<PathBuf>,
    },
    /// Switch to canary (download latest pre-release from GitHub)
    Canary,
}

// ── persistence ───────────────────────────────────────────────────────────────

fn orbit_state_dir() -> PathBuf {
    orbit_core::data_paths::orbit_state_dir()
}

fn mode_file() -> PathBuf {
    orbit_state_dir().join("mode")
}

fn dev_path_file() -> PathBuf {
    orbit_state_dir().join("dev_path")
}

pub fn current_mode() -> String {
    let s = fs::read_to_string(mode_file()).unwrap_or_default();
    let s = s.trim();
    match s {
        "" => "stable".to_string(),
        // Legacy: "beta" was renamed to "canary".
        "beta" => "canary".to_string(),
        other => other.to_string(),
    }
}

fn write_mode(mode: &str) -> Result<()> {
    ensure_data_dir()?;
    fs::write(mode_file(), mode)?;
    Ok(())
}

fn read_dev_path() -> Option<PathBuf> {
    fs::read_to_string(dev_path_file())
        .ok()
        .map(|s| PathBuf::from(s.trim()))
}

fn write_dev_path(path: &std::path::Path) -> Result<()> {
    ensure_data_dir()?;
    fs::write(dev_path_file(), path.to_string_lossy().as_bytes())?;
    Ok(())
}

fn ensure_data_dir() -> Result<()> {
    fs::create_dir_all(orbit_state_dir())?;
    Ok(())
}

// ── platform ──────────────────────────────────────────────────────────────────

fn platform_artifact() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "orbit-linux-x86_64",
        ("linux", "aarch64") => "orbit-linux-aarch64",
        ("macos", "x86_64") => "orbit-macos-x86_64",
        ("macos", "aarch64") => "orbit-macos-aarch64",
        _ => "orbit-linux-x86_64",
    }
}

fn make_binary_url(tag: &str) -> String {
    format!(
        "https://github.com/tensiply/orbit/releases/download/{tag}/{}",
        platform_artifact()
    )
}

fn make_checksums_url(tag: &str) -> String {
    format!("https://github.com/tensiply/orbit/releases/download/{tag}/checksums.txt")
}

// ── entry point ───────────────────────────────────────────────────────────────

pub async fn run(args: ModeArgs) -> Result<()> {
    match args.command {
        ModeCommand::Status => status(),
        ModeCommand::Stable => switch_to_stable().await,
        ModeCommand::Dev { path } => switch_to_dev(path),
        ModeCommand::Canary => switch_to_canary().await,
    }
}

// ── status ────────────────────────────────────────────────────────────────────

fn status() -> Result<()> {
    // A non-stable binary tracks its own channel; the stable binary follows the
    // runtime `orbit mode` switch. Display the effective one so it matches how
    // updates actually resolve.
    let mode = if Channel::current() == Channel::Stable {
        current_mode()
    } else {
        Channel::current().as_str().to_string()
    };
    let install_dir = UserConfig::load().install_dir_expanded();
    let orbit_bin = install_dir.join(format!("orbit{}", Channel::current().home_suffix()));

    match mode.as_str() {
        "dev" => {
            print!("  mode:   dev");
            match orbit_bin.read_link() {
                Ok(target) => println!(" → {}", target.display()),
                Err(_) => println!(" (symlink missing — run `orbit mode dev <path>`)"),
            }
            if let Some(saved) = read_dev_path() {
                println!("  saved:  {}", saved.display());
                if !saved.exists() {
                    println!("  warning: saved path does not exist (build first)");
                }
            }
        }
        "canary" => {
            println!("  mode:   canary (pre-release)");
            println!("  binary: {}", orbit_bin.display());
        }
        _ => {
            println!("  mode:   stable");
            println!("  binary: {}", orbit_bin.display());
        }
    }
    Ok(())
}

// ── stable ────────────────────────────────────────────────────────────────────

async fn switch_to_stable() -> Result<()> {
    let client = build_client()?;

    print!("  Fetching latest stable version... ");
    let _ = std::io::stdout().flush();
    let tag = match update_check::fetch_latest_tag(&client).await {
        Ok(t) => t,
        Err(e) if e.to_string().contains("404") || e.to_string().contains("Not Found") => {
            println!("no releases found");
            write_mode("stable")?;
            println!("  No GitHub releases available — marking current build as stable.");
            println!("  Run `make install` to rebuild from source.");
            return Ok(());
        }
        Err(e) => {
            println!("failed");
            return Err(anyhow::anyhow!("Could not fetch release info: {e}"));
        }
    };
    println!("{tag}");

    let artifact = platform_artifact().to_string();
    let binary_url = make_binary_url(&tag);
    let checksums_url = make_checksums_url(&tag);

    let install_path = UserConfig::load().install_dir_expanded().join("orbit");
    let backup = backup_binary(&install_path)?;

    let result = update::update_binary_to(
        &client,
        &binary_url,
        &checksums_url,
        &artifact,
        &tag,
        &install_path,
    )
    .await;

    match result {
        Ok(()) => {
            cleanup_backup(backup);
            write_mode("stable")?;
            println!("  Switched to stable mode ({tag}).");
            Ok(())
        }
        Err(e) => {
            restore_backup(backup, &install_path);
            Err(e)
        }
    }
}

// ── dev ───────────────────────────────────────────────────────────────────────

fn switch_to_dev(path_arg: Option<PathBuf>) -> Result<()> {
    let build_path = match path_arg {
        Some(p) => {
            if p.is_absolute() {
                p
            } else {
                std::env::current_dir()?.join(p)
            }
        }
        None => read_dev_path()
            .context("No local build path saved. Run `orbit mode dev <path>` to set one.")?,
    };

    let build_path = build_path.canonicalize().with_context(|| {
        format!(
            "Path does not exist: {}\nRun `cargo build --release` first.",
            build_path.display()
        )
    })?;

    let install_dir = UserConfig::load().install_dir_expanded();
    let orbit_bin = install_dir.join("orbit");

    if orbit_bin.symlink_metadata().is_ok() {
        fs::remove_file(&orbit_bin)?;
    }
    std::os::unix::fs::symlink(&build_path, &orbit_bin)?;
    write_dev_path(&build_path)?;
    write_mode("dev")?;

    println!("  Switched to dev mode.");
    println!("  {} → {}", orbit_bin.display(), build_path.display());
    println!();
    println!("  The symlink updates automatically when you rebuild.");
    println!("  Run `orbit mode stable` or `orbit mode canary` to switch back.");
    Ok(())
}

// ── canary ────────────────────────────────────────────────────────────────────

async fn switch_to_canary() -> Result<()> {
    let client = build_client()?;

    print!("  Fetching latest pre-release... ");
    let _ = std::io::stdout().flush();
    let tag = match update_check::fetch_latest_prerelease_tag(&client).await {
        Ok(t) => t,
        Err(e) if e.to_string().contains("404") || e.to_string().contains("Not Found") => {
            println!("no pre-releases found");
            write_mode("canary")?;
            println!("  No GitHub pre-releases available — marking current build as canary.");
            println!("  Run `make install` to rebuild from source.");
            return Ok(());
        }
        Err(e) => {
            println!("failed");
            return Err(anyhow::anyhow!("{e}"));
        }
    };
    println!("{tag}");

    let artifact = platform_artifact().to_string();
    let binary_url = make_binary_url(&tag);
    let checksums_url = make_checksums_url(&tag);

    let install_path = UserConfig::load().install_dir_expanded().join("orbit");
    let backup = backup_binary(&install_path)?;

    let result = update::update_binary_to(
        &client,
        &binary_url,
        &checksums_url,
        &artifact,
        &tag,
        &install_path,
    )
    .await;

    match result {
        Ok(()) => {
            cleanup_backup(backup);
            write_mode("canary")?;
            println!("  Switched to canary mode ({tag}).");
            Ok(())
        }
        Err(e) => {
            restore_backup(backup, &install_path);
            Err(e)
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build HTTP client")
}

/// Move the current binary/symlink to a `.bak` side-car before downloading a
/// replacement. Returns the backup path if something was moved.
fn backup_binary(install_path: &std::path::Path) -> Result<Option<PathBuf>> {
    if install_path.symlink_metadata().is_err() {
        return Ok(None);
    }
    let backup = install_path.with_extension("bak");
    fs::rename(install_path, &backup)
        .with_context(|| format!("failed to back up {}", install_path.display()))?;
    Ok(Some(backup))
}

/// Remove the backup after a successful install.
fn cleanup_backup(backup: Option<PathBuf>) {
    if let Some(p) = backup {
        let _ = fs::remove_file(p);
    }
}

/// Restore the backup after a failed install.
fn restore_backup(backup: Option<PathBuf>, install_path: &std::path::Path) {
    if let Some(p) = backup {
        if let Err(e) = fs::rename(&p, install_path) {
            eprintln!("  warning: could not restore previous binary: {e}");
            eprintln!("  backup is at: {}", p.display());
        } else {
            eprintln!("  Previous binary restored.");
        }
    }
}
