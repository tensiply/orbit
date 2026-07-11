use anyhow::Result;
use clap::{Parser, Subcommand};
use orbit_core::{user_config::UserConfig, workspace_config::WorkspaceConfig};

pub mod auto_update;
pub mod banner;
pub mod commands;
mod update_check;

#[derive(Debug, Parser)]
#[command(name = "orbit", about = "AI ecosystem CLI", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Skip background update checks for this invocation
    #[arg(long, global = true)]
    pub no_update: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check and manage engine authentication
    Auth(commands::auth::AuthArgs),
    /// Manage AI engines: list, install, update, info
    Engines(commands::engines::EnginesArgs),
    /// First-time setup: write config and install the binary
    Setup(commands::setup::SetupArgs),
    /// Get, set, or list config values
    Config(commands::config::ConfigArgs),
    /// Manage the active orbit binary mode (stable / dev / beta)
    Mode(commands::mode::ModeArgs),
    /// Clone the governance repository into the AI root
    Init(commands::init::InitArgs),
    /// Sync governance configs and/or update the binary
    Update(commands::update::UpdateArgs),
    /// Launch an AI engine (opencode/gemini/claude) with full context resolution
    Launch(commands::launch::LaunchArgs),
    /// Manage active sessions
    Session(commands::session::SessionArgs),
    /// Interact with the orbit daemon
    Daemon(commands::daemon::DaemonArgs),
    /// Browse workspace / tenant / project / repository hierarchy
    Ls(commands::ls::LsArgs),
    /// Print shell completion script
    Completions(commands::completions::CompletionsArgs),
    /// Manage MCP servers: list, enable, disable, info
    Mcp(commands::mcp::McpArgs),
    /// Manage orbit plugins (install, list, wrap)
    Plugins(commands::plugins::PluginsArgs),
    /// Quick snapshot of current state: workspace, engine, scope, daemon, sessions
    Status(commands::status::StatusArgs),
    /// Run environment diagnostics
    Doctor(commands::doctor::DoctorArgs),
    /// Save a context snapshot for the current scope to the governance repo
    Snapshot(commands::snapshot::SnapshotArgs),
    /// Manage Jira integration: board mappings, orgs
    Jira(commands::jira::JiraArgs),
    /// Store and retrieve secrets in the OS keychain
    Secret(commands::secret::SecretArgs),
    /// Manage env vars in orbit.json at any scope level
    Env(commands::env::EnvArgs),
    /// Create and manage autonomous execution plans
    Plan(commands::plan::PlanArgs),
    /// Search and manage plan run memory
    Memory(commands::memory::MemoryArgs),
    /// Configure and test desktop notifications
    Notify(commands::notify::NotifyArgs),
    /// Inspect context layers, instructions, and MCP for the current scope
    Context(commands::context::ContextArgs),
    /// Generate or install man pages for orbit commands
    Man(commands::man::ManArgs),
}

impl Cli {
    pub fn parse_dev() -> Self {
        Self::parse()
    }
}

fn needs_setup(cmd: &Option<Commands>) -> bool {
    !matches!(
        cmd,
        Some(Commands::Setup(_))
            | Some(Commands::Completions(_))
            | Some(Commands::Man(_))
            | Some(Commands::Update(_))
            | None
    )
}

pub async fn run(cli: Cli) -> Result<()> {
    // First-run detection: guide new users before any command runs.
    if needs_setup(&cli.command) && !UserConfig::path().exists() {
        eprintln!();
        eprintln!("  No config found. Run `orbit setup` to get started.");
        eprintln!();
        std::process::exit(1);
    }

    let user_cfg = UserConfig::load();
    let ws_cfg = {
        let ai_root = user_cfg.ai_root_expanded();
        WorkspaceConfig::load(&ai_root)
    };

    // Notify if a previous background update installed a new binary.
    auto_update::print_pending_notification();

    // Fire-and-forget background update (governance pull + binary).
    auto_update::spawn(ws_cfg.clone(), user_cfg.clone(), cli.no_update);

    match cli.command {
        Some(Commands::Auth(args)) => commands::auth::run(args),
        Some(Commands::Engines(args)) => commands::engines::run(args),
        Some(Commands::Setup(args)) => commands::setup::run(args).await,
        Some(Commands::Config(args)) => commands::config::run(args),
        Some(Commands::Mode(args)) => commands::mode::run(args).await,
        Some(Commands::Init(args)) => commands::init::run(args).await,
        Some(Commands::Update(args)) => commands::update::run(args).await,
        Some(Commands::Launch(args)) => commands::launch::run(args).await,
        Some(Commands::Session(args)) => commands::session::run(args).await,
        Some(Commands::Daemon(args)) => commands::daemon::run(args).await,
        Some(Commands::Ls(args)) => commands::ls::run(args),
        Some(Commands::Completions(args)) => commands::completions::run(args),
        Some(Commands::Mcp(args)) => commands::mcp::run(args),
        Some(Commands::Plugins(args)) => commands::plugins::run(args),
        Some(Commands::Status(args)) => commands::status::run(args).await,
        Some(Commands::Doctor(args)) => commands::doctor::run(args),
        Some(Commands::Snapshot(args)) => commands::snapshot::run(args),
        Some(Commands::Jira(args)) => commands::jira::run(args),
        Some(Commands::Secret(args)) => commands::secret::run(args),
        Some(Commands::Env(args)) => commands::env::run(args),
        Some(Commands::Plan(args)) => commands::plan::run(args).await,
        Some(Commands::Memory(args)) => commands::memory::run(args),
        Some(Commands::Notify(args)) => commands::notify::run(args),
        Some(Commands::Context(args)) => commands::context::run(args),
        Some(Commands::Man(args)) => commands::man::run(args),
        None => {
            update_check::check_and_print(&ws_cfg).await;

            if let Some(params) = orbit_tui::run().await? {
                use orbit_core::engine::Engine;
                commands::launch::run(commands::launch::LaunchArgs {
                    workspace: if params.workspace.is_empty() {
                        None
                    } else {
                        Some(params.workspace)
                    },
                    tenant: if params.tenant.is_empty() {
                        None
                    } else {
                        Some(params.tenant)
                    },
                    project: if params.project.is_empty() {
                        None
                    } else {
                        Some(params.project)
                    },
                    repository: if params.repository.is_empty() {
                        None
                    } else {
                        Some(params.repository)
                    },
                    engine: Some(match params.engine {
                        Engine::Opencode => commands::launch::CliEngine::Opencode,
                        Engine::Gemini => commands::launch::CliEngine::Gemini,
                        Engine::Claude => commands::launch::CliEngine::Claude,
                    }),
                    dry_run: false,
                    no_tmux: params.no_tmux,
                    task: params.task_context.as_ref().map(|t| t.key.clone()),
                    no_task: params.task_context.is_none(),
                })
                .await?;
            }
            Ok(())
        }
    }
}
