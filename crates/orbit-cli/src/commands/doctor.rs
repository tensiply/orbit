use anyhow::Result;
use clap::Args;
use orbit_core::{ipc::socket_path, user_config::UserConfig, workspace_config::WorkspaceConfig};
use std::process::Command;

#[derive(Debug, Args)]
pub struct DoctorArgs;

pub fn run(_args: DoctorArgs) -> Result<()> {
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();
    let ws_cfg = WorkspaceConfig::load(&ai_root);

    println!("orbit doctor\n");

    // ── engines ───────────────────────────────────────────────────────────────
    section("engines");
    check_engine("opencode", "npm install -g opencode-ai");
    check_engine("gemini", "npm install -g @google/gemini-cli");
    check_engine("claude", "npm install -g @anthropic-ai/claude-code");
    println!();

    // ── dependencies ──────────────────────────────────────────────────────────
    section("dependencies");
    check("tmux", check_bin("tmux"), Some("https://github.com/tmux/tmux/wiki/Installing"));
    check("node", check_bin("node"), Some("https://nodejs.org"));
    println!();

    // ── workspace ─────────────────────────────────────────────────────────────
    section("workspace");
    let ai_root_str = ai_root.display().to_string();
    if ai_root.is_dir() {
        let label = if ai_root.join(".git").is_dir() {
            format!("AI root (git)   {ai_root_str}")
        } else {
            format!("AI root (local) {ai_root_str}")
        };
        check(&label, Ok::<(), &str>(()), None);
    } else {
        check(
            &format!("AI root         {ai_root_str}"),
            Err("not found — run `orbit init` or `orbit setup`"),
            None,
        );
    }
    if !ws_cfg.governance.url.is_empty() {
        println!("    governance: {}", ws_cfg.governance.url);
    }
    println!();

    // ── config ────────────────────────────────────────────────────────────────
    section("config");
    println!("  {} {}", dim("file"), UserConfig::path().display());
    println!("  {} {}", dim("engine.default        "), user_cfg.engine.default);
    let tenant = if user_cfg.engine.default_tenant.is_empty() {
        "(none)".to_string()
    } else {
        user_cfg.engine.default_tenant.clone()
    };
    println!("  {} {}", dim("engine.default_tenant "), tenant);
    println!("  {} {}", dim("workspace.ai_root      "), user_cfg.workspace.ai_root.display());
    println!("  {} {}", dim("install.dir            "), user_cfg.install.dir.display());
    println!();

    // ── daemon ────────────────────────────────────────────────────────────────
    section("daemon");
    let sock = socket_path();
    check(
        "daemon",
        if sock.exists() {
            Ok(())
        } else {
            Err("not running — start with `orbit daemon start`")
        },
        None,
    );

    println!();

    // ── binary ────────────────────────────────────────────────────────────────
    section("binary");
    let install_dir = user_cfg.install_dir_expanded();
    let orbit_bin = install_dir.join("orbit");
    check(
        &format!("install dir  {}", install_dir.display()),
        if install_dir.is_dir() {
            Ok(())
        } else {
            Err("directory not found")
        },
        None,
    );
    check(
        &format!("orbit binary {}", orbit_bin.display()),
        if orbit_bin.exists() {
            Ok(())
        } else {
            Err("binary not found")
        },
        None,
    );

    println!();
    Ok(())
}

fn section(title: &str) {
    println!("\x1b[1m{title}\x1b[0m");
}

fn dim(s: &str) -> String {
    format!("\x1b[2m{s}\x1b[0m")
}

fn check<E: std::fmt::Display>(label: &str, result: Result<(), E>, hint: Option<&str>) {
    match result {
        Ok(()) => println!("  \x1b[32m✓\x1b[0m  {label}"),
        Err(e) => {
            println!("  \x1b[31m✗\x1b[0m  {label}  — {e}");
            if let Some(h) = hint {
                println!("      \x1b[2m{h}\x1b[0m");
            }
        }
    }
}

fn check_engine(bin: &str, install_cmd: &str) {
    if Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        println!("  \x1b[32m✓\x1b[0m  {bin}");
    } else {
        println!("  \x1b[31m✗\x1b[0m  {bin}  — not found in PATH");
        println!("      \x1b[2minstall: {install_cmd}\x1b[0m");
    }
}

fn check_bin(bin: &str) -> Result<(), &'static str> {
    if Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err("not found in PATH")
    }
}
