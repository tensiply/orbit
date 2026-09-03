use anyhow::{Result, bail};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
#[command(about = "Open the orbit desktop app")]
pub struct DesktopArgs {
    /// Force using the dev build from the current repo
    #[arg(long)]
    pub dev: bool,
}

pub fn run(args: DesktopArgs) -> Result<()> {
    let binary = if args.dev || is_dev_binary() {
        dev_binary()?
    } else {
        stable_binary()?
    };

    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(&binary).exec();
    bail!("failed to launch {}: {err}", binary.display())
}

/// True when the current executable is orbit-dev (binary name contains "dev").
fn is_dev_binary() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .map(|name| name.contains("dev"))
        .unwrap_or(false)
}

/// Dev binary: look for orbit-desktop next to the current exe (same target/{profile}/ dir).
fn dev_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine exe directory"))?;
    let candidate = dir.join("orbit-desktop");
    if candidate.exists() {
        return Ok(candidate);
    }
    bail!(
        "orbit-desktop dev binary not found at {}.\n  Build it with: cargo build -p orbit-desktop",
        candidate.display()
    )
}

/// Stable binary: orbit-desktop resolved via PATH (respects any install location).
fn stable_binary() -> Result<PathBuf> {
    // Check ~/.local/bin first (default make install) to give a better error if missing
    let local_bin = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".local/bin/orbit-desktop"));

    if let Some(ref p) = local_bin
        && p.exists()
    {
        return Ok(p.clone());
    }

    // Let the OS resolve via PATH — exec() will fail with ENOENT if not found
    // We check explicitly so we can show a helpful message
    let in_path = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(PathBuf::from)
        .any(|dir| dir.join("orbit-desktop").exists());

    if in_path {
        return Ok(PathBuf::from("orbit-desktop"));
    }

    bail!(
        "orbit-desktop is not installed.\n  Build and install: make desktop-build && make desktop-install\n  Or run the dev build:  orbit-dev desktop"
    )
}
