use anyhow::{Result, bail};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
#[command(about = "Open the orbit desktop app")]
pub struct DesktopArgs {
    /// Launch the dev desktop build (dev-orbit-desktop) instead of the stable app
    #[arg(long)]
    pub dev: bool,
}

pub fn run(args: DesktopArgs) -> Result<()> {
    // The desktop app ships as a separate product (repo orbit-desktop). The dev
    // build installs as `dev-orbit-desktop`, the stable one as `orbit-desktop`.
    // A dev CLI (dev-orbit) launches the dev app so both stay on ~/.orbit-dev.
    let dev = args.dev || is_dev_binary();
    let binary = if dev {
        resolve("dev-orbit-desktop").ok_or_else(|| {
            anyhow::anyhow!(
                "dev-orbit-desktop is not installed.\n  Build and link it: cd orbit-desktop && make dev-install"
            )
        })?
    } else {
        resolve("orbit-desktop").ok_or_else(|| {
            anyhow::anyhow!(
                "orbit-desktop is not installed.\n  Build and install: cd orbit-desktop && make bundle install\n  Or run the dev build: orbit desktop --dev"
            )
        })?
    };

    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(&binary).exec();
    bail!("failed to launch {}: {err}", binary.display())
}

/// True when the current executable is a dev build (binary name contains "dev").
fn is_dev_binary() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .map(|name| name.contains("dev"))
        .unwrap_or(false)
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
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(name).exists())
        })
        .unwrap_or(false);

    on_path.then(|| PathBuf::from(name))
}
