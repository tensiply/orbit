use std::{
    io::{self, IsTerminal, Write},
    process::Command,
};

// ── availability ──────────────────────────────────────────────────────────────

pub fn available() -> bool {
    which("tmux")
}

pub fn already_inside() -> bool {
    std::env::var("TMUX").is_ok()
}

pub fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── auto-install ──────────────────────────────────────────────────────────────

/// Ensures tmux is available. If it isn't and stdin is a TTY, prompts the
/// user to install it via the system package manager.
/// Returns `true` if tmux is available after this call.
pub fn ensure_available() -> bool {
    if available() {
        return true;
    }

    // Non-interactive context (CI, piped stdin) — fall back silently
    if !io::stdin().is_terminal() {
        return false;
    }

    let Some(pm) = detect_package_manager() else {
        eprintln!(
            "  tmux not found and no supported package manager detected.\n\
             Install tmux manually to enable session persistence."
        );
        return false;
    };

    print!("  tmux not found — install it for session persistence? [Y/n]: ");
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    let answer = input.trim().to_lowercase();
    if !answer.is_empty() && answer != "y" && answer != "yes" {
        return false;
    }

    println!("  Installing tmux via {}…", pm.name());
    if run_install(&pm) {
        println!("  tmux installed.");
        true
    } else {
        eprintln!(
            "  Installation failed — launching without tmux.\n\
             Install manually: {}",
            pm.install_hint()
        );
        false
    }
}

// ── package manager detection ─────────────────────────────────────────────────

enum PackageManager {
    Apt,    // Debian / Ubuntu
    Dnf,    // Fedora / RHEL 8+
    Yum,    // CentOS / RHEL 7
    Pacman, // Arch
    Zypper, // openSUSE
    Apk,    // Alpine
    Brew,   // macOS
}

impl PackageManager {
    fn name(&self) -> &'static str {
        match self {
            Self::Apt => "apt-get",
            Self::Dnf => "dnf",
            Self::Yum => "yum",
            Self::Pacman => "pacman",
            Self::Zypper => "zypper",
            Self::Apk => "apk",
            Self::Brew => "brew",
        }
    }

    fn install_hint(&self) -> &'static str {
        match self {
            Self::Apt => "sudo apt-get install -y tmux",
            Self::Dnf => "sudo dnf install -y tmux",
            Self::Yum => "sudo yum install -y tmux",
            Self::Pacman => "sudo pacman -Sy --noconfirm tmux",
            Self::Zypper => "sudo zypper install -y tmux",
            Self::Apk => "apk add tmux",
            Self::Brew => "brew install tmux",
        }
    }
}

fn detect_package_manager() -> Option<PackageManager> {
    if which("apt-get") {
        return Some(PackageManager::Apt);
    }
    if which("dnf") {
        return Some(PackageManager::Dnf);
    }
    if which("yum") {
        return Some(PackageManager::Yum);
    }
    if which("pacman") {
        return Some(PackageManager::Pacman);
    }
    if which("zypper") {
        return Some(PackageManager::Zypper);
    }
    if which("apk") {
        return Some(PackageManager::Apk);
    }
    if which("brew") {
        return Some(PackageManager::Brew);
    }
    None
}

fn run_install(pm: &PackageManager) -> bool {
    let root = is_root();
    let has_sudo = which("sudo");

    let (bin, args): (&str, Vec<&str>) = match pm {
        PackageManager::Apt => ("apt-get", vec!["install", "-y", "tmux"]),
        PackageManager::Dnf => ("dnf", vec!["install", "-y", "tmux"]),
        PackageManager::Yum => ("yum", vec!["install", "-y", "tmux"]),
        PackageManager::Pacman => ("pacman", vec!["-Sy", "--noconfirm", "tmux"]),
        PackageManager::Zypper => ("zypper", vec!["install", "-y", "tmux"]),
        PackageManager::Apk => ("apk", vec!["add", "tmux"]),
        PackageManager::Brew => ("brew", vec!["install", "tmux"]),
    };

    // brew and apk run as the current user; others need root
    let use_sudo = !root && has_sudo && !matches!(pm, PackageManager::Brew | PackageManager::Apk);

    let status = if use_sudo {
        Command::new("sudo").arg(bin).args(&args).status()
    } else {
        Command::new(bin).args(&args).status()
    };

    status.map(|s| s.success()).unwrap_or(false)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_root() -> bool {
    // Check effective UID via the `id` command — avoids adding a libc dep
    let out = Command::new("id").arg("-u").output().ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_inside_reflects_env() {
        // Temporarily clear TMUX so this test is not affected by the calling environment
        let original = std::env::var("TMUX").ok();
        unsafe { std::env::remove_var("TMUX") };
        assert!(!already_inside());
        if let Some(val) = original {
            unsafe { std::env::set_var("TMUX", val) };
        }
    }

    #[test]
    fn is_root_does_not_panic() {
        let _ = is_root(); // just verify it runs without crashing
    }

    #[test]
    fn detect_package_manager_does_not_panic() {
        let _ = detect_package_manager();
    }
}
