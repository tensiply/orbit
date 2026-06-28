use anyhow::Result;
use clap::{Parser, Subcommand};
use orbit_core::{user_config::UserConfig, workspace_config::WorkspaceConfig};

pub mod commands;
mod update_check;

#[derive(Debug, Parser)]
#[command(name = "orbit", about = "AI ecosystem CLI", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// First-time setup: write config and install the binary
    Setup(commands::setup::SetupArgs),
    /// Switch between stable and development mode
    Dev(commands::dev::DevArgs),
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
}

impl Cli {
    pub fn parse_dev() -> Self {
        Self::parse()
    }
}

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Setup(args)) => commands::setup::run(args).await,
        Some(Commands::Dev(args)) => commands::dev::run(args).await,
        Some(Commands::Init(args)) => commands::init::run(args).await,
        Some(Commands::Update(args)) => commands::update::run(args).await,
        Some(Commands::Launch(args)) => commands::launch::run(args).await,
        Some(Commands::Session(args)) => commands::session::run(args).await,
        Some(Commands::Daemon(args)) => commands::daemon::run(args).await,
        None => {
            let ws_cfg = {
                let ai_root = UserConfig::load().ai_root_expanded();
                WorkspaceConfig::load(&ai_root)
            };
            update_check::check_and_print(&ws_cfg).await;

            if let Some(params) = orbit_tui::run().await? {
                use orbit_core::engine::Engine;
                commands::launch::run(commands::launch::LaunchArgs {
                    workspace: None,
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
                    engine: match params.engine {
                        Engine::Opencode => commands::launch::CliEngine::Opencode,
                        Engine::Gemini => commands::launch::CliEngine::Gemini,
                        Engine::Claude => commands::launch::CliEngine::Claude,
                    },
                    dry_run: false,
                    no_tmux: params.no_tmux,
                })
                .await?;
            }
            Ok(())
        }
    }
}
