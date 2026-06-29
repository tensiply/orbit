use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use orbit_core::user_config::UserConfig;
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
    /// Switch to beta (download latest pre-release from GitHub)
    Beta,
}

// ── persistence ───────────────────────────────────────────────────────────────

fn orbit_data_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".local/share/orbit"))
        .unwrap_or_else(|| PathBuf::from("/tmp/orbit"))
}

fn mode_file() -> PathBuf {
    orbit_data_dir().join("mode")
}

fn dev_path_file() -> PathBuf {
    orbit_data_dir().join("dev_path")
}

pub fn current_mode() -> String {
    let s = fs::read_to_string(mode_file()).unwrap_or_default();
    let s = s.trim();
    if s.is_empty() {
        "stable".to_string()
    } else {
        s.to_string()
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
    fs::create_dir_all(orbit_data_dir())?;
    Ok(())
}

// ── platform ──────────────────────────────────────────────────────────────────

fn platform_artifact() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "orbit-linux-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "orbit-linux-aarch64";
    #[allow(unreachable_code)]
    "orbit"
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
        ModeCommand::Beta => switch_to_beta().await,
    }
}

// ── status ────────────────────────────────────────────────────────────────────

fn status() -> Result<()> {
    let mode = current_mode();
    let install_dir = UserConfig::load().install_dir_expanded();
    let orbit_bin = install_dir.join("orbit");

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
        "beta" => {
            println!("  mode:   beta (pre-release)");
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
    let tag = update_check::fetch_latest_tag(&client).await.map_err(|e| {
        println!("failed");
        anyhow::anyhow!("Could not fetch release info: {e}")
    })?;
    println!("{tag}");

    let artifact = platform_artifact().to_string();
    let binary_url = make_binary_url(&tag);
    let checksums_url = make_checksums_url(&tag);

    let install_path = UserConfig::load().install_dir_expanded().join("orbit");
    remove_binary_or_symlink()?;
    update::update_binary_to(&client, &binary_url, &checksums_url, &artifact, &tag, &install_path).await?;

    write_mode("stable")?;
    println!("  Switched to stable mode ({tag}).");
    Ok(())
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

    remove_binary_or_symlink()?;
    std::os::unix::fs::symlink(&build_path, &orbit_bin)?;
    write_dev_path(&build_path)?;
    write_mode("dev")?;

    println!("  Switched to dev mode.");
    println!("  {} → {}", orbit_bin.display(), build_path.display());
    println!();
    println!("  The symlink updates automatically when you rebuild.");
    println!("  Run `orbit mode stable` or `orbit mode beta` to switch back.");
    Ok(())
}

// ── beta ──────────────────────────────────────────────────────────────────────

async fn switch_to_beta() -> Result<()> {
    let client = build_client()?;

    print!("  Fetching latest pre-release... ");
    let _ = std::io::stdout().flush();
    let tag = update_check::fetch_latest_prerelease_tag(&client)
        .await
        .map_err(|e| {
            println!("failed");
            anyhow::anyhow!("{e}")
        })?;
    println!("{tag}");

    let artifact = platform_artifact().to_string();
    let binary_url = make_binary_url(&tag);
    let checksums_url = make_checksums_url(&tag);

    let install_path = UserConfig::load().install_dir_expanded().join("orbit");
    remove_binary_or_symlink()?;
    update::update_binary_to(&client, &binary_url, &checksums_url, &artifact, &tag, &install_path).await?;

    write_mode("beta")?;
    println!("  Switched to beta mode ({tag}).");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build HTTP client")
}

fn remove_binary_or_symlink() -> Result<()> {
    let orbit_bin = UserConfig::load().install_dir_expanded().join("orbit");
    if orbit_bin.symlink_metadata().is_ok() {
        fs::remove_file(&orbit_bin)?;
    }
    Ok(())
}
