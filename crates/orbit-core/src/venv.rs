use anyhow::bail;
use std::{fs, path::PathBuf, process::Command};

// ── paths ─────────────────────────────────────────────────────────────────────

pub fn venv_dir() -> PathBuf {
    // The venv is persistent across sessions. Orbit overrides XDG_DATA_HOME for
    // session isolation, so we bypass it here and always resolve from the real
    // home directory. ORBIT_DATA_HOME can override if explicitly set.
    if let Ok(p) = std::env::var("ORBIT_DATA_HOME") {
        return PathBuf::from(p).join("venv");
    }
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".local/share/orbit/venv"))
        .unwrap_or_else(|| PathBuf::from("/tmp/orbit/venv"))
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

    let python = find_system_python().ok_or_else(|| {
        anyhow::anyhow!(
            "Python 3 is required for this plugin.\n\n\
             Install it with:\n\
             \n  Ubuntu/Debian:  sudo apt install python3 python3-venv\
             \n  macOS:          brew install python3\
             \n  Other:          https://python.org/downloads"
        )
    })?;

    let dir = venv_dir();
    fs::create_dir_all(&dir)?;
    println!("  Creating orbit Python venv at {}…", dir.display());

    let status = Command::new(&python)
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

/// Find the best Python 3 interpreter for creating the venv.
///
/// Prefers the system Python (`/usr/bin/python3`) over a Homebrew/pyenv install
/// because the venv inherits the interpreter's RPATH and ABI. A Homebrew Python
/// on Linux has an RPATH that points to linuxbrew dirs only, making it unable to
/// load system-installed native libs (pango, fribidi, …) needed by weasyprint.
fn find_system_python() -> Option<PathBuf> {
    // Candidate order: system Debian/Ubuntu path first, then PATH fallback.
    let candidates = [
        "/usr/bin/python3",
        "/usr/bin/python3.11",
        "/usr/bin/python3.12",
        "/usr/bin/python3.10",
        "python3",
    ];
    for candidate in &candidates {
        let ok = Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}
