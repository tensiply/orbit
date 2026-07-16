use anyhow::bail;
use std::{fs, path::PathBuf, process::Command};

use crate::data_paths::orbit_data_root;

// ── paths ─────────────────────────────────────────────────────────────────────

pub fn venv_dir() -> PathBuf {
    orbit_data_root().join("venv")
}

pub fn venv_bin(name: &str) -> PathBuf {
    venv_dir().join("bin").join(name)
}

pub fn venv_exists() -> bool {
    venv_bin("python3").exists()
}

// ── lifecycle ─────────────────────────────────────────────────────────────────

/// Ensure the orbit-managed Python venv exists, creating it if necessary.
///
/// Errors with a human-readable message if Python 3 is not installed.
pub fn ensure_venv() -> anyhow::Result<()> {
    if venv_exists() {
        return Ok(());
    }

    let python_ok = Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());

    if !python_ok {
        bail!(
            "Python 3 is required for this plugin.\n\n\
             Install it with:\n\
             \n  Ubuntu/Debian:  sudo apt install python3 python3-venv\
             \n  macOS:          brew install python3\
             \n  Other:          https://python.org/downloads"
        );
    }

    let dir = venv_dir();
    fs::create_dir_all(&dir)?;
    println!("  Creating orbit Python venv at {}…", dir.display());

    let status = Command::new("python3")
        .args(["-m", "venv", dir.to_str().unwrap_or_default()])
        .status()?;

    if !status.success() {
        bail!(
            "Failed to create Python venv at {}.\n\
             On Ubuntu/Debian try: sudo apt install python3-venv",
            dir.display()
        );
    }

    Ok(())
}
