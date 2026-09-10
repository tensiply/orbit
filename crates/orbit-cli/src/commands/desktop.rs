use anyhow::{Result, bail};
use clap::Args;
use orbit_core::channel::Channel;
use std::path::PathBuf;

#[derive(Debug, Args)]
#[command(about = "Open the orbit desktop app")]
pub struct DesktopArgs {
    /// Launch the dev desktop build (dev-orbit-desktop) instead of the running channel's app
    #[arg(long)]
    pub dev: bool,
}

pub fn run(args: DesktopArgs) -> Result<()> {
    // The desktop app ships as a separate product (repo orbit-desktop) with one
    // binary per channel. Each CLI launches the desktop of its own channel so
    // both stay on the same ~/.orbit{,-canary,-dev} home. `--dev` forces the dev
    // build regardless of the running channel.
    let channel = if args.dev {
        Channel::Dev
    } else {
        Channel::current()
    };
    let name = desktop_binary_name(channel);
    let binary = resolve(name).ok_or_else(|| anyhow::anyhow!(not_installed_hint(channel)))?;

    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(&binary).exec();
    bail!("failed to launch {}: {err}", binary.display())
}

/// Installed binary name of the desktop app for a channel. The dev and canary
/// builds are prefixed so all three channels can coexist in `~/.local/bin`.
fn desktop_binary_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "orbit-desktop",
        Channel::Canary => "canary-orbit-desktop",
        Channel::Dev => "dev-orbit-desktop",
    }
}

/// Guidance shown when the desktop binary for a channel is not installed.
fn not_installed_hint(channel: Channel) -> String {
    match channel {
        Channel::Stable => "orbit-desktop is not installed.\n  Build and install: cd orbit-desktop && make bundle install\n  Or run the dev build: orbit desktop --dev".to_string(),
        Channel::Canary => "canary-orbit-desktop is not installed.\n  Build and link it: cd orbit-desktop && make canary-install".to_string(),
        Channel::Dev => "dev-orbit-desktop is not installed.\n  Build and link it: cd orbit-desktop && make dev-install".to_string(),
    }
}

/// Resolve a desktop binary by name: `~/.local/bin` first (default install
/// location), then anywhere on PATH. Returns `None` if not found.
fn resolve(name: &str) -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join(".local/bin").join(name);
        if p.exists() {
            return Some(p);
        }
    }

    let on_path = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).exists()))
        .unwrap_or(false);

    on_path.then(|| PathBuf::from(name))
}
