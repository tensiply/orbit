use anyhow::Result;
use clap::Args;
use orbit_core::{
    catalog, ipc::socket_path, user_config::UserConfig, workspace_config::WorkspaceConfig,
};
use std::process::Command;

use super::auth::{AuthStatus, detect_auth};
use super::plugins::print_plugins_section;

#[derive(Debug, Args)]
pub struct DoctorArgs;

pub fn run(_args: DoctorArgs) -> Result<()> {
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();
    let ws_cfg = WorkspaceConfig::load(&ai_root);

    println!("orbit doctor\n");

    // ── engines ───────────────────────────────────────────────────────────────
    section("engines");
    for engine in catalog::engines() {
        check_engine_full(&engine);
    }
    println!();

    // ── dependencies ──────────────────────────────────────────────────────────
    section("dependencies");
    check(
        "tmux",
        check_bin("tmux"),
        Some("https://github.com/tmux/tmux/wiki/Installing"),
    );
    check("node", check_bin("node"), Some("https://nodejs.org"));
    println!();

    // ── plugins ───────────────────────────────────────────────────────────────
    print_plugins_section();

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
    println!(
        "  {} {}",
        dim("engine.default          "),
        user_cfg.engine.default
    );
    let tenant = if user_cfg.engine.default_tenant.is_empty() {
        "(none)".to_string()
    } else {
        user_cfg.engine.default_tenant.clone()
    };
    println!("  {} {}", dim("engine.default_tenant   "), tenant);
    let workspace = if user_cfg.engine.default_workspace.is_empty() {
        "(none)".to_string()
    } else {
        user_cfg.engine.default_workspace.clone()
    };
    println!("  {} {}", dim("engine.default_workspace"), workspace);
    println!(
        "  {} {}",
        dim("workspace.ai_root       "),
        user_cfg.workspace.ai_root.display()
    );
    println!(
        "  {} {}",
        dim("install.dir             "),
        user_cfg.install.dir.display()
    );
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

fn check_engine_full(engine: &orbit_core::catalog::EngineEntry) {
    let installed = check_bin(&engine.bin).is_ok();
    let auth = detect_auth(engine);

    let install_mark = if installed {
        "\x1b[32m✓\x1b[0m"
    } else {
        "\x1b[31m✗\x1b[0m"
    };
    let auth_tag = match &auth {
        AuthStatus::Configured(signal) => {
            format!("  \x1b[32m✓ auth\x1b[0m  \x1b[2m{signal}\x1b[0m")
        }
        AuthStatus::NotConfigured if installed => {
            "  \x1b[33m○ auth\x1b[0m  \x1b[2mnot configured\x1b[0m".to_string()
        }
        AuthStatus::NotConfigured => String::new(),
    };

    println!("  {install_mark}  {}{auth_tag}", engine.name);

    if !installed {
        println!(
            "      \x1b[2minstall: npm install -g {}\x1b[0m",
            engine.npm_package
        );
    } else if matches!(auth, AuthStatus::NotConfigured) {
        println!("      \x1b[2mauth: orbit auth {}\x1b[0m", engine.name);
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
