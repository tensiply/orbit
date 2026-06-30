use anyhow::Result;
use clap::{Args, ValueEnum};
use orbit_core::engine::Engine;
use orbit_engine::{
    config,
    launcher::{self, LaunchOptions},
    resolver,
};

/// Clap-facing engine selector. Kept separate from `orbit_core::Engine` so that
/// `orbit-core` never has to depend on clap.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliEngine {
    Opencode,
    Gemini,
    Claude,
}

impl From<CliEngine> for Engine {
    fn from(e: CliEngine) -> Self {
        match e {
            CliEngine::Opencode => Engine::Opencode,
            CliEngine::Gemini => Engine::Gemini,
            CliEngine::Claude => Engine::Claude,
        }
    }
}

#[derive(Debug, Args)]
pub struct LaunchArgs {
    /// Workspace name — case-insensitive, resolves to ~/WORKSPACE.
    /// Use "." to auto-resolve scope from the current working directory.
    pub workspace: Option<String>,

    /// Tenant name within the workspace (default: AI)
    pub tenant: Option<String>,

    /// Project name within the tenant
    pub project: Option<String>,

    /// Repository name within the project
    pub repository: Option<String>,

    /// AI engine to launch [default: opencode]
    #[arg(short, long, value_enum, default_value = "opencode")]
    pub engine: CliEngine,

    /// Print the resolved config without launching the engine (useful for debugging)
    #[arg(long)]
    pub dry_run: bool,

    /// Launch the engine directly without wrapping in a tmux session
    #[arg(long)]
    pub no_tmux: bool,
}

pub async fn run(args: LaunchArgs) -> Result<()> {
    let engine = Engine::from(args.engine);

    // "." means resolve scope from cwd automatically
    let scope = if args.workspace.as_deref() == Some(".") {
        resolver::resolve_from_cwd()?
    } else {
        resolver::resolve(resolver::ResolveArgs {
            workspace: args.workspace,
            tenant: args.tenant,
            project: args.project,
            repository: args.repository,
        })?
    };

    tracing::debug!(
        engine = engine.as_str(),
        work_dir = %scope.work_dir.display(),
        tenant = %scope.tenant,
        project = %scope.project,
        repository = %scope.repository,
        global_mode = scope.global_mode,
        "scope resolved"
    );

    // Load and merge config from all scope layers
    let merged = config::load(&scope, engine)?;

    tracing::debug!(
        instructions = merged.instructions.len(),
        mcp_servers = merged.mcp.len(),
        "config loaded"
    );

    // Dry-run: print rendered config and exit without launching
    if args.dry_run {
        let rendered = launcher::render::render(&merged, engine);
        println!("{}", serde_json::to_string_pretty(&rendered)?);
        return Ok(());
    }

    // no_tmux sessions cannot go through the daemon (daemon can't exec into the terminal)
    if !args.no_tmux {
        // Try daemon first; fall back to direct launch if unavailable
        if !orbit_client::ipc::is_available() {
            ensure_daemon_running().await;
        }

        if orbit_client::ipc::is_available() {
            match orbit_client::ipc::launch_session(
                scope
                    .workspace_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string()),
                if scope.tenant.is_empty() {
                    None
                } else {
                    Some(scope.tenant.clone())
                },
                if scope.project.is_empty() {
                    None
                } else {
                    Some(scope.project.clone())
                },
                if scope.repository.is_empty() {
                    None
                } else {
                    Some(scope.repository.clone())
                },
                engine.as_str(),
                false,
            )
            .await
            {
                Ok(info) => {
                    // Session spawned in background — attach to the tmux window
                    return attach_tmux(&info.tmux_name);
                }
                Err(e) => {
                    tracing::debug!("daemon launch failed ({e}), falling back to direct exec");
                }
            }
        }
    }

    // Direct fallback (or no_tmux requested)
    launcher::launch(
        &scope,
        &merged,
        engine,
        LaunchOptions {
            no_tmux: args.no_tmux,
        },
    )
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn ensure_daemon_running() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn();
    // Give it a moment to bind the socket
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

fn attach_tmux(session_name: &str) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let cmd = if std::env::var("TMUX").is_ok() {
        "switch-client"
    } else {
        "attach-session"
    };
    let err = std::process::Command::new("tmux")
        .args([cmd, "-t", session_name])
        .exec();
    anyhow::bail!("failed to exec tmux {cmd}: {err}");
}
