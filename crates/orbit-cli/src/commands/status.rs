use anyhow::Result;
use clap::Args;
use orbit_client::ipc as client_ipc;
use orbit_core::{
    catalog, ipc::socket_path, user_config::UserConfig, workspace_config::WorkspaceConfig,
};
use orbit_engine::resolver;
use std::time::Duration;

use super::auth::{AuthStatus, detect_auth};

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Output as JSON (useful for scripts, tmux status line, shell prompts)
    #[arg(long)]
    pub json: bool,
}

// ── collected state ───────────────────────────────────────────────────────────

struct StatusData {
    workspace_name: String,
    workspace_path: String,
    workspace_exists: bool,
    workspace_is_git: bool,
    engine: String,
    engine_installed: bool,
    engine_auth: bool,
    engine_auth_signal: Option<String>,
    tenant: String,
    scope_level: String,
    scope_label: String,
    daemon_running: bool,
    sessions_active: usize,
    version: String,
}

// ── entry point ───────────────────────────────────────────────────────────────

pub async fn run(args: StatusArgs) -> Result<()> {
    let data = collect().await;

    if args.json {
        print_json(&data);
    } else {
        print_human(&data);
    }

    // Exit 1 if any critical issue
    if !data.workspace_exists || !data.engine_installed {
        std::process::exit(1);
    }

    Ok(())
}

// ── data collection ───────────────────────────────────────────────────────────

async fn collect() -> StatusData {
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();

    // Workspace
    let workspace_name = ai_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AI".to_string());
    let workspace_path = ai_root.display().to_string();
    let workspace_exists = ai_root.is_dir();
    let workspace_is_git = ai_root.join(".git").is_dir();

    // Engine
    let engine_name = user_cfg.engine.default.clone();
    let engine_entry = catalog::engine_by_name(&engine_name);
    let engine_installed = engine_entry
        .as_ref()
        .map(|e| bin_available(&e.bin))
        .unwrap_or(false);
    let (engine_auth, engine_auth_signal) = engine_entry
        .as_ref()
        .map(|e| match detect_auth(e) {
            AuthStatus::Configured(s) => (true, Some(s)),
            AuthStatus::NotConfigured => (false, None),
        })
        .unwrap_or((false, None));

    // Tenant
    let tenant = user_cfg.engine.default_tenant.clone();

    // Scope from cwd
    let (scope_level, scope_label) = resolve_scope();

    // Daemon + sessions (async, with timeout)
    let (daemon_running, sessions_active) = daemon_status().await;

    // Version
    let version = env!("CARGO_PKG_VERSION").to_string();

    // Suppress unused warning on ws_cfg
    let _ = WorkspaceConfig::load(&ai_root);

    StatusData {
        workspace_name,
        workspace_path,
        workspace_exists,
        workspace_is_git,
        engine: engine_name,
        engine_installed,
        engine_auth,
        engine_auth_signal,
        tenant,
        scope_level,
        scope_label,
        daemon_running,
        sessions_active,
        version,
    }
}

fn resolve_scope() -> (String, String) {
    match resolver::resolve_from_cwd() {
        Err(_) => ("global".to_string(), String::new()),
        Ok(scope) if scope.global_mode => ("global".to_string(), String::new()),
        Ok(scope) => {
            let mut parts = Vec::new();
            if !scope.tenant.is_empty() {
                parts.push(scope.tenant.clone());
            }
            if !scope.project.is_empty() {
                parts.push(scope.project.clone());
            }
            if !scope.repository.is_empty() {
                parts.push(scope.repository.clone());
            }

            let level = if !scope.repository.is_empty() {
                "repo"
            } else if !scope.project.is_empty() {
                "project"
            } else if !scope.tenant.is_empty() {
                "tenant"
            } else {
                "workspace"
            };

            (level.to_string(), parts.join("/"))
        }
    }
}

async fn daemon_status() -> (bool, usize) {
    if !socket_path().exists() {
        return (false, 0);
    }
    match tokio::time::timeout(Duration::from_millis(150), client_ipc::status()).await {
        Ok(Ok(info)) => (true, info.session_count),
        _ => (false, 0),
    }
}

// ── human output ──────────────────────────────────────────────────────────────

fn print_human(d: &StatusData) {
    println!("orbit status\n");

    let label_w = 12usize;

    // workspace
    let ws_detail = if !d.workspace_exists {
        "\x1b[31mnot found\x1b[0m".to_string()
    } else if d.workspace_is_git {
        format!("\x1b[2m{} (git)\x1b[0m", d.workspace_path)
    } else {
        format!("\x1b[2m{}\x1b[0m", d.workspace_path)
    };
    row(
        "workspace",
        label_w,
        &format!("{}  {}", d.workspace_name, ws_detail),
    );

    // engine
    let install_tag = if d.engine_installed {
        "\x1b[32m✓ installed\x1b[0m"
    } else {
        "\x1b[31m✗ not installed\x1b[0m"
    };
    let auth_tag = match (&d.engine_auth, &d.engine_auth_signal) {
        (true, Some(s)) => format!("  \x1b[32m✓ auth\x1b[0m  \x1b[2m{s}\x1b[0m"),
        (true, None) => "  \x1b[32m✓ auth\x1b[0m".to_string(),
        (false, _) => format!(
            "  \x1b[33m○ auth\x1b[0m  \x1b[2morbit auth {}\x1b[0m",
            d.engine
        ),
    };
    row(
        "engine",
        label_w,
        &format!("{}  {install_tag}{auth_tag}", d.engine),
    );

    // tenant
    if !d.tenant.is_empty() {
        row("tenant", label_w, &d.tenant);
    }

    // scope
    let scope_detail = if d.scope_label.is_empty() {
        String::new()
    } else {
        format!("  \x1b[2m{}\x1b[0m", d.scope_label)
    };
    row(
        "scope",
        label_w,
        &format!("{}{}", d.scope_level, scope_detail),
    );

    // daemon
    let daemon_detail = if d.daemon_running {
        let s = if d.sessions_active == 1 { "" } else { "s" };
        format!(
            "\x1b[32mrunning\x1b[0m  \x1b[2m{} session{s} active\x1b[0m",
            d.sessions_active
        )
    } else {
        "\x1b[2mstopped\x1b[0m  \x1b[2morbit daemon start\x1b[0m".to_string()
    };
    row("daemon", label_w, &daemon_detail);

    // version
    row("version", label_w, &format!("v{}", d.version));

    println!();
}

fn row(label: &str, label_w: usize, rest: &str) {
    let pad = " ".repeat(label_w.saturating_sub(label.len()));
    println!("  \x1b[2m{label}\x1b[0m{pad}  {rest}");
}

// ── JSON output ───────────────────────────────────────────────────────────────

fn print_json(d: &StatusData) {
    let obj = serde_json::json!({
        "workspace": d.workspace_name,
        "workspace_path": d.workspace_path,
        "workspace_exists": d.workspace_exists,
        "workspace_is_git": d.workspace_is_git,
        "engine": d.engine,
        "engine_installed": d.engine_installed,
        "engine_auth": d.engine_auth,
        "engine_auth_signal": d.engine_auth_signal,
        "tenant": d.tenant,
        "scope_level": d.scope_level,
        "scope_label": d.scope_label,
        "daemon_running": d.daemon_running,
        "sessions_active": d.sessions_active,
        "version": d.version,
    });
    println!("{}", serde_json::to_string_pretty(&obj).unwrap_or_default());
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn bin_available(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
