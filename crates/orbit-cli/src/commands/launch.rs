use anyhow::Result;
use clap::{Args, ValueEnum};
use orbit_core::{engine::Engine, user_config::UserConfig};
use orbit_engine::{
    config::{self, ScopeReport},
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

    /// AI engine to launch (default: reads engine.default from config)
    #[arg(short, long, value_enum)]
    pub engine: Option<CliEngine>,

    /// Print the resolved config without launching the engine (useful for debugging)
    #[arg(long)]
    pub dry_run: bool,

    /// Launch the engine directly without wrapping in a tmux session
    #[arg(long)]
    pub no_tmux: bool,

    /// Attach a specific Jira issue to this session (e.g. ORBIT-123)
    #[arg(long)]
    pub task: Option<String>,

    /// Skip the Jira task prompt for this launch
    #[arg(long)]
    pub no_task: bool,
}

pub async fn run(args: LaunchArgs) -> Result<()> {
    let engine = match args.engine {
        Some(e) => Engine::from(e),
        None => {
            let cfg = UserConfig::load();
            cfg.engine
                .default
                .parse::<Engine>()
                .unwrap_or(Engine::Opencode)
        }
    };

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

    // Resolve Jira task (if plugin enabled and not skipped)
    let task_context = super::jira::resolve_task_for_launch(
        args.task.as_deref(),
        args.no_task,
    );

    // Dry-run: print human-readable scope + context report
    if args.dry_run {
        crate::banner::print();
        let (_, report) = config::inspect(&scope, engine)?;
        print_dry_run(&scope, engine, &merged, &report, task_context.as_ref());
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
        task_context.as_ref(),
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

// ── dry-run report ────────────────────────────────────────────────────────────

fn print_dry_run(
    scope: &orbit_core::context::OrbitScope,
    engine: Engine,
    merged: &orbit_engine::config::MergedConfig,
    report: &ScopeReport,
    task: Option<&orbit_core::jira::TaskContext>,
) {
    let ok = "\x1b[32m✓\x1b[0m";
    let skip = "\x1b[2m·\x1b[0m";
    let warn = "\x1b[33m~\x1b[0m";

    let bold = |s: &str| format!("\x1b[1m{s}\x1b[0m");
    let dim = |s: &str| format!("\x1b[2m{s}\x1b[0m");

    // ── scope ─────────────────────────────────────────────────────────────────
    println!("{}", bold("scope"));
    let lw = 12usize;
    let row = |label: &str, val: &str| {
        let pad = " ".repeat(lw.saturating_sub(label.len()));
        println!("  {}{}  {}", dim(label), pad, val);
    };
    row("workspace", &scope.workspace_root.to_string_lossy());
    if !scope.tenant.is_empty() {
        row("tenant", &scope.tenant);
    }
    if !scope.project.is_empty() {
        row("project", &scope.project);
    }
    if !scope.repository.is_empty() {
        row("repository", &scope.repository);
    }
    row("engine", engine.as_str());
    row("work dir", &scope.work_dir.to_string_lossy());
    let config_file = orbit_engine::launcher::runtime::config_file_path(scope, engine);
    let context_file = orbit_engine::launcher::runtime::context_file_path(scope, engine);
    let exec_cmd = match engine {
        Engine::Claude => {
            let ctx = context_file
                .as_ref()
                .map(|p| format!(" --append-system-prompt-file {}", p.display()))
                .unwrap_or_default();
            format!("claude --mcp-config {}{ctx}", config_file.display())
        }
        Engine::Opencode => format!("OPENCODE_CONFIG={} opencode", config_file.display()),
        Engine::Gemini => format!(
            "GEMINI_CLI_SYSTEM_SETTINGS_PATH={} gemini",
            config_file.display()
        ),
    };
    row("exec", &exec_cmd);
    println!();

    // ── task context ──────────────────────────────────────────────────────────
    println!("{}", bold("task"));
    match task {
        Some(t) => {
            let pad = " ".repeat(lw.saturating_sub("task".len()));
            println!("  {}{}  {} — {}", dim("task"), pad, t.key, t.summary);
            let pad = " ".repeat(lw.saturating_sub("status".len()));
            println!("  {}{}  {}", dim("status"), pad, t.status);
            let pad = " ".repeat(lw.saturating_sub("priority".len()));
            println!("  {}{}  {}", dim("priority"), pad, t.priority);
        }
        None => {
            println!("  {}  none", skip);
        }
    }
    println!();

    // ── config layers ─────────────────────────────────────────────────────────
    println!("{}", bold("config layers"));
    for entry in report.config_layers.iter().filter(|e| e.exists) {
        println!(
            "  {}  {}  {}",
            ok,
            entry.path.display(),
            dim(&format!("({})", entry.label)),
        );
    }
    println!();

    // ── agent overlays ────────────────────────────────────────────────────────
    let loaded_overlays: Vec<_> = report.agent_overlay_dirs.iter().filter(|e| e.exists).collect();
    if !loaded_overlays.is_empty() {
        println!("{}", bold("agent overlays"));
        for entry in &loaded_overlays {
            println!(
                "  {}  {}/  {}",
                ok,
                entry.path.display(),
                dim(&format!("({})", entry.label)),
            );
        }
        println!();
    }

    // ── MCP layers ────────────────────────────────────────────────────────────
    println!("{}", bold("mcp layers"));
    for entry in report.mcp_layers.iter().filter(|e| e.exists) {
        println!(
            "  {}  {}  {}",
            ok,
            entry.path.display(),
            dim(&format!("({})", entry.label)),
        );
    }
    println!();

    // ── instructions (engine-specific display) ────────────────────────────────
    let loaded_instructions: Vec<_> = report.instructions.iter().filter(|(_, e)| *e).collect();

    match engine {
        Engine::Opencode => {
            println!(
                "{}  {}  {}",
                bold("instructions"),
                dim(&format!("({})", loaded_instructions.len())),
                dim("→ injected as instructions array"),
            );
            for (path, _) in &loaded_instructions {
                println!("  {}  {}", ok, path.display());
            }
            println!();
        }
        Engine::Gemini => {
            println!(
                "{}  {}  {}",
                bold("instructions"),
                dim(&format!("({})", loaded_instructions.len())),
                dim("→ parent dirs passed to context.includeDirectories"),
            );
            for (path, _) in &loaded_instructions {
                println!("  {}  {}", skip, path.display());
            }
            println!();

            // Show the actual include dirs Gemini will use
            const GEMINI_FILENAMES: &[&str] = &["README.md", "GEMINI.md", "CONTEXT.md", "AGENTS.md"];
            let include_dirs = gemini_include_dirs(
                &loaded_instructions.iter().map(|(p, _)| p.as_path()).collect::<Vec<_>>(),
            );
            let home = directories::BaseDirs::new()
                .map(|b| b.home_dir().to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("/"));
            println!(
                "{}  {}  {}",
                bold("include dirs"),
                dim(&format!("({})", include_dirs.len())),
                dim(&format!("loads: {}", GEMINI_FILENAMES.join(", "))),
            );
            for dir in &include_dirs {
                let expanded = if let Ok(rest) = dir.strip_prefix("~") {
                    home.join(rest)
                } else {
                    dir.to_path_buf()
                };
                let has_match = GEMINI_FILENAMES.iter().any(|f| expanded.join(f).exists());
                let marker = if has_match { ok } else { warn };
                println!("  {}  {}", marker, dir.display());
            }
            println!();
        }
        Engine::Claude => {
            println!(
                "{}  {}  {}",
                bold("instructions"),
                dim(&format!("({})", loaded_instructions.len())),
                dim("→ injected via --append-system-prompt-file"),
            );
            for (path, _) in &loaded_instructions {
                println!("  {}  {}", ok, path.display());
            }
            println!();

            // Show actual CLAUDE.md files Claude Code will load via traversal,
            // plus the @-refs each file injects so context is fully visible.
            let claude_files = find_claude_md_files(&scope.work_dir);
            let total_refs: usize = claude_files
                .iter()
                .map(|f| parse_claude_at_refs(f).len())
                .sum();
            println!(
                "{}  {}",
                bold("claude context"),
                dim(&format!(
                    "CLAUDE.md traversal: work dir → home  ({} files, {} @refs)",
                    claude_files.len(),
                    total_refs,
                )),
            );
            if claude_files.is_empty() {
                println!("  {}  none found", skip);
            } else {
                for f in &claude_files {
                    println!("  {}  {}", ok, f.display());
                    for r in parse_claude_at_refs(f) {
                        let marker = if r.exists() { ok } else { warn };
                        println!("     {}  {}", marker, r.display());
                    }
                }
            }
            println!();
        }
    }

    // ── mcp servers ───────────────────────────────────────────────────────────
    println!(
        "{}  {}",
        bold("mcp servers"),
        dim(&format!("({})", report.mcp_servers.len()))
    );
    if report.mcp_servers.is_empty() {
        println!("  {}  none", skip);
    } else {
        let name_w = report
            .mcp_servers
            .iter()
            .map(|(n, _)| n.len())
            .max()
            .unwrap_or(0);
        for (name, cmd) in &report.mcp_servers {
            let pad = " ".repeat(name_w.saturating_sub(name.len()));
            println!("  {}  {}{pad}  {}", ok, name, cmd.join(" "));
        }
    }
    println!();

    let _ = merged;
}

fn parse_claude_at_refs(claude_md: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(content) = std::fs::read_to_string(claude_md) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed.strip_prefix('@').and_then(|rest| {
                let path = rest.trim();
                if path.starts_with('/') {
                    Some(std::path::PathBuf::from(path))
                } else {
                    None
                }
            })
        })
        .collect()
}

fn find_claude_md_files(work_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("/"))
        });
    let mut files = Vec::new();
    let mut current = work_dir.to_path_buf();
    loop {
        let candidate = current.join(".claude").join("CLAUDE.md");
        if candidate.exists() {
            files.push(candidate);
        }
        if current == home {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    files
}

fn gemini_include_dirs(paths: &[&std::path::Path]) -> Vec<std::path::PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut dirs = Vec::new();
    for p in paths {
        if let Some(parent) = p.parent() {
            if seen.insert(parent.to_path_buf()) {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    dirs
}
