//! Engine availability bootstrap.
//!
//! On Windows the AI engines (currently Claude) ship via npm, so they depend on
//! Node.js and land as a `claude.cmd` shim rather than a native binary. This
//! module mirrors [`super::tmux::ensure_available`]: it detects whether the
//! engine's CLI is installed and, in an interactive terminal, offers to install
//! it (Node.js via winget, then the Claude CLI via npm). If something is missing
//! and we cannot install it, it degrades with a clear message rather than
//! failing silently — the launch continues and the engine's own "not found"
//! error surfaces.

use orbit_core::engine::Engine;

/// Ensure the engine's CLI is available before launching. Returns `true` if the
/// engine is (or was made) available.
///
/// On unix this is a no-op: users manage their own engine install, matching the
/// existing behavior (only tmux is auto-installed there).
#[cfg(not(windows))]
pub fn ensure_available(_engine: Engine) -> bool {
    true
}

#[cfg(windows)]
pub fn ensure_available(engine: Engine) -> bool {
    use std::io::{self, IsTerminal, Write};

    // Only Claude has an npm-based install path we can bootstrap today.
    if engine != Engine::Claude {
        return true;
    }
    if which("claude") {
        return true;
    }

    // Non-interactive context (daemon, CI, piped stdin) — degrade silently; the
    // engine's own error surfaces when the launch reaches it.
    if !io::stdin().is_terminal() {
        return false;
    }

    if !which("winget") {
        eprintln!(
            "  Claude CLI not found and winget is unavailable.\n\
             Install Node.js from https://nodejs.org, then run: npm install -g @anthropic-ai/claude-code"
        );
        return false;
    }

    print!("  Claude CLI not found — install Node.js + Claude now? [Y/n]: ");
    io::stdout().flush().ok();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    let answer = input.trim().to_lowercase();
    if !answer.is_empty() && answer != "y" && answer != "yes" {
        return false;
    }

    if !which("node") || !which("npm") {
        println!("  Installing Node.js via winget…");
        if !install_node() {
            eprintln!(
                "  Node.js install failed.\n\
                 Install it manually from https://nodejs.org, then run: npm install -g @anthropic-ai/claude-code"
            );
            return false;
        }
    }

    println!("  Installing Claude CLI via npm…");
    if install_claude() && which("claude") {
        println!("  Claude CLI installed.");
        true
    } else {
        eprintln!(
            "  Claude CLI install failed. If Node.js was just installed, open a new\n\
             terminal and run: npm install -g @anthropic-ai/claude-code"
        );
        false
    }
}

#[cfg(windows)]
fn which(bin: &str) -> bool {
    std::process::Command::new("where")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn install_node() -> bool {
    // `-e --id` pins the exact package; the `--accept-*` flags keep winget from
    // blocking on interactive agreement prompts.
    std::process::Command::new("winget")
        .args([
            "install",
            "-e",
            "--id",
            "OpenJS.NodeJS",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn install_claude() -> bool {
    // npm is itself a `.cmd` shim, so route through `cmd /c` (CreateProcess
    // cannot run a `.cmd` directly).
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/c", "npm", "install", "-g", "@anthropic-ai/claude-code"]);
    // winget updates the machine PATH, but this process's environment is stale,
    // so a freshly-installed npm is not yet resolvable. Prepend Node's default
    // install dir so the one-shot install works without restarting the terminal.
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let nodejs = format!("{pf}\\nodejs");
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{nodejs};{path}"));
    }
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn ensure_available_is_noop_on_unix() {
        // On unix the bootstrap never blocks a launch, for any engine.
        assert!(ensure_available(Engine::Claude));
        assert!(ensure_available(Engine::Gemini));
    }
}
