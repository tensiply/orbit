use anyhow::Result;
use clap::{Parser, Subcommand};
use orbit_core::{channel::Channel, user_config::UserConfig, workspace_config::WorkspaceConfig};

pub mod auto_update;
pub mod banner;
pub mod commands;
pub mod output;
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
    /// Track and query session activity history
    Activity(commands::activity::ActivityArgs),
    /// Manage AI engines: list, install, update, auth
    Engines(commands::engines::EnginesArgs),
    /// First-time setup: write config and install the binary
    Setup(commands::setup::SetupArgs),
    /// Get, set, or list config values
    Config(commands::config::ConfigArgs),
    /// Manage the active orbit binary mode (stable / dev / canary)
    Mode(commands::mode::ModeArgs),
    /// Clone the governance repository into the AI root
    Init(commands::init::InitArgs),
    /// Sync governance configs and/or update the binary
    Update(commands::update::UpdateArgs),
    /// Launch an AI engine (opencode/gemini/claude) with full context resolution.
    /// Use "recent" as workspace to pick from recently visited scopes.
    Launch(commands::launch::LaunchArgs),
    /// Manage active sessions
    Session(commands::session::SessionArgs),
    /// Interact with the orbit daemon
    Daemon(commands::daemon::DaemonArgs),
    /// Open the orbit desktop app
    Desktop(commands::desktop::DesktopArgs),
    /// Browse workspace / tenant / project / repository hierarchy
    Ls(commands::ls::LsArgs),
    /// Print shell completion script
    Completions(commands::completions::CompletionsArgs),
    /// Manage MCP servers: list, enable, disable, info
    Mcp(commands::mcp::McpArgs),
    /// Manage orbit plugins (install, list, wrap)
    Plugins(commands::plugins::PluginsArgs),
    /// Manage commands: list catalog, enable or disable per scope
    Command(commands::command::CommandArgs),
    /// Manage Claude Code engine hooks (Stop, Notification, etc.)
    Hooks(commands::hooks::HooksArgs),
    /// Quick snapshot of current state: workspace, engine, scope, daemon, sessions
    Status(commands::status::StatusArgs),
    /// Run environment diagnostics
    Doctor(commands::doctor::DoctorArgs),
    /// Save a context snapshot for the current scope to the governance repo
    Snapshot(commands::snapshot::SnapshotArgs),
    /// Manage Jira integration: board mappings, orgs
    Jira(commands::jira::JiraArgs),
    /// Scan, list and validate governance scopes across all workspaces
    Scope(commands::scope::ScopeArgs),
    /// Store and retrieve secrets in the OS keychain
    Secret(commands::secret::SecretArgs),
    /// Manage env vars in orbit.json at any scope level
    Env(commands::env::EnvArgs),
    /// Create and manage autonomous execution plans
    Plan(Box<commands::plan::PlanArgs>),
    /// Search and manage plan run memory
    Memory(commands::memory::MemoryArgs),
    /// Configure and test desktop notifications
    Notify(commands::notify::NotifyArgs),
    /// Inspect context layers, instructions, and MCP for the current scope
    Context(commands::context::ContextArgs),
    /// Generate or install man pages for orbit commands
    Man(commands::man::ManArgs),
    /// Manage registered workspaces (multi-workspace support)
    Workspace(commands::workspace::WorkspaceArgs),
    /// Create, list, update and delete internal tasks
    Task(commands::task::TaskArgs),
    /// Share this orbit instance over LAN via TCP + mDNS
    Serve(commands::serve::ServeArgs),
    /// Discover orbit instances on the local network via mDNS
    Discover(commands::discover::DiscoverArgs),
    /// Generate documents (PDF, HTML, DOCX, XLSX, CSV) from markdown or data
    Document(commands::document::DocumentArgs),
    /// Generate images (PNG, JPEG, WEBP) from HTML templates or AI
    Image(commands::image::ImageArgs),
    /// Generate SVG files from templates or raw SVG content
    Svg(commands::svg::SvgArgs),
    /// Print shell integration (eval in .zshrc/.bashrc)
    #[command(name = "shell-init")]
    ShellInit(commands::shell_init::ShellInitArgs),
    /// View the architecture catalog for a scope (services, databases, integrations, etc.)
    Architecture(commands::architecture::ArchitectureArgs),
}

/// Shared entrypoint for every channel binary (`orbit`, `orbit-canary`,
/// `orbit-dev`). Publishes the channel to the rest of the process via
/// `ORBIT_CHANNEL` (so `orbit-core` resolves the right home, keychain, and
/// branding), initializes logging at the channel's default verbosity, and runs
/// the CLI. Each bin's `main()` is a one-liner delegating here.
pub async fn run_channel(channel: Channel) -> Result<()> {
    // Set before any orbit code reads ORBIT_CHANNEL and before worker threads
    // run user code — no concurrent env access at this point. Respect an
    // explicit override (tests / nested launches) if one is already set.
    if std::env::var_os("ORBIT_CHANNEL").is_none() {
        // Safety: single-threaded startup, no other thread touches the env yet.
        unsafe { std::env::set_var("ORBIT_CHANNEL", channel.as_str()) };
    }

    let default_filter = match channel {
        Channel::Dev => "orbit=debug,orbit_engine=debug,orbit_daemon=debug",
        Channel::Stable | Channel::Canary => "orbit=info",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default_filter.into()),
        )
        .init();

    if std::env::args().any(|a| a == "-h" || a == "--help") {
        banner::print();
    }
    run(Cli::parse()).await
}

fn needs_setup(cmd: &Option<Commands>) -> bool {
    !matches!(
        cmd,
        Some(Commands::Setup(_))
            | Some(Commands::Completions(_))
            | Some(Commands::Man(_))
            | Some(Commands::Update(_))
            | Some(Commands::ShellInit(_))
            | None
    )
}

fn shows_banner(cmd: &Option<Commands>) -> bool {
    // Completions, Man, and ShellInit produce machine-readable output; Launch handles its own
    // banner for --dry-run; None opens the TUI which manages its own display.
    !matches!(
        cmd,
        Some(Commands::Completions(_))
            | Some(Commands::Man(_))
            | Some(Commands::Launch(_))
            | Some(Commands::ShellInit(_))
            | None
    )
}

pub async fn run(cli: Cli) -> Result<()> {
    if shows_banner(&cli.command) {
        banner::print();
    }

    // First-run detection: guide new users before any command runs.
    if needs_setup(&cli.command) && !UserConfig::path().exists() {
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
        Some(Commands::Activity(args)) => commands::activity::run(args),
        Some(Commands::Engines(args)) => commands::engines::run(args),
        Some(Commands::Setup(args)) => commands::setup::run(args).await,
        Some(Commands::Config(args)) => commands::config::run(args),
        Some(Commands::Mode(args)) => commands::mode::run(args).await,
        Some(Commands::Init(args)) => commands::init::run(args).await,
        Some(Commands::Update(args)) => commands::update::run(args).await,
        Some(Commands::Launch(args)) => commands::launch::run(args).await,
        Some(Commands::Session(args)) => commands::session::run(args).await,
        Some(Commands::Daemon(args)) => commands::daemon::run(args).await,
        Some(Commands::Desktop(args)) => commands::desktop::run(args),
        Some(Commands::Ls(args)) => commands::ls::run(args),
        Some(Commands::Completions(args)) => commands::completions::run(args),
        Some(Commands::Mcp(args)) => commands::mcp::run(args),
        Some(Commands::Plugins(args)) => commands::plugins::run(args),
        Some(Commands::Command(args)) => commands::command::run(args),
        Some(Commands::Hooks(args)) => commands::hooks::run(args),
        Some(Commands::Status(args)) => commands::status::run(args).await,
        Some(Commands::Doctor(args)) => commands::doctor::run(args),
        Some(Commands::Snapshot(args)) => commands::snapshot::run(args),
        Some(Commands::Jira(args)) => commands::jira::run(args),
        Some(Commands::Scope(args)) => commands::scope::run(args),
        Some(Commands::Secret(args)) => commands::secret::run(args),
        Some(Commands::Env(args)) => commands::env::run(args),
        Some(Commands::Plan(args)) => commands::plan::run(*args).await,
        Some(Commands::Memory(args)) => commands::memory::run(args),
        Some(Commands::Notify(args)) => commands::notify::run(args),
        Some(Commands::Context(args)) => commands::context::run(args),
        Some(Commands::Man(args)) => commands::man::run(args),
        Some(Commands::Workspace(args)) => commands::workspace::run(args),
        Some(Commands::Task(args)) => commands::task::run(args),
        Some(Commands::Serve(args)) => commands::serve::run(args).await,
        Some(Commands::Discover(args)) => commands::discover::run(args),
        Some(Commands::Document(args)) => commands::document::run(args),
        Some(Commands::Image(args)) => commands::image::run(args),
        Some(Commands::Svg(args)) => commands::svg::run(args),
        Some(Commands::ShellInit(args)) => commands::shell_init::run(args),
        Some(Commands::Architecture(args)) => commands::architecture::run(args),
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
                    new_session: false,
                    print_work_dir: false,
                })
                .await?;
            }
            Ok(())
        }
    }
}
