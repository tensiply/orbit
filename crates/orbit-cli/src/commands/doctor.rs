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

    check("tmux     ", check_bin("tmux"));
    check("opencode ", check_bin("opencode"));
    check("gemini   ", check_bin("gemini"));
    check("claude   ", check_bin("claude"));

    println!();

    let ai_root_str = ai_root.display().to_string();
    if ai_root.is_dir() {
        if ai_root.join(".git").is_dir() {
            check(
                &format!("AI root (git)   {ai_root_str}"),
                Ok::<(), &str>(()),
            );
        } else {
            check(
                &format!("AI root (local) {ai_root_str}"),
                Ok::<(), &str>(()),
            );
        }
    } else {
        check(
            &format!("AI root         {ai_root_str}"),
            Err("not found — run `orbit init` or `orbit setup`"),
        );
    }

    if !ws_cfg.governance.url.is_empty() {
        println!("  governance: {}", ws_cfg.governance.url);
    }

    println!();

    let sock = socket_path();
    check(
        "daemon",
        if sock.exists() {
            Ok(())
        } else {
            Err("not running — start with `orbit daemon start`")
        },
    );

    println!();

    let install_dir = user_cfg.install_dir_expanded();
    let orbit_bin = install_dir.join("orbit");
    check(
        &format!("install dir  {}", install_dir.display()),
        if install_dir.is_dir() {
            Ok(())
        } else {
            Err("directory not found")
        },
    );
    check(
        &format!("orbit binary {}", orbit_bin.display()),
        if orbit_bin.exists() {
            Ok(())
        } else {
            Err("binary not found")
        },
    );

    println!();
    Ok(())
}

fn check<E: std::fmt::Display>(label: &str, result: Result<(), E>) {
    match result {
        Ok(()) => println!("  \x1b[32m✓\x1b[0m  {label}"),
        Err(e) => println!("  \x1b[31m✗\x1b[0m  {label}  — {e}"),
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
