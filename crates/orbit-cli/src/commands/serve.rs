use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::ipc::{ProjectRole, Request, Response};

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[command(subcommand)]
    pub command: Option<ServeCommand>,

    /// TCP port to listen on
    #[arg(long, default_value = "7373", global = true)]
    pub port: u16,

    /// Maximum role to grant network peers
    #[arg(long, value_enum, default_value = "observer", global = true)]
    pub role: ServeRole,

    /// Instance name for mDNS (default: hostname)
    #[arg(long, global = true)]
    pub name: Option<String>,

    /// Block the terminal until Ctrl+C (default: exits immediately, daemon keeps serving)
    #[arg(long, global = true)]
    pub foreground: bool,
}

#[derive(Debug, Subcommand)]
pub enum ServeCommand {
    /// Start sharing — daemon keeps serving after this command exits (default)
    Start,
    /// Stop sharing and close the TCP listener + mDNS announcement
    Stop,
    /// Show connected peers and sharing status
    Status,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ServeRole {
    Observer,
    Contributor,
}

pub async fn run(args: ServeArgs) -> Result<()> {
    match args.command.unwrap_or(ServeCommand::Start) {
        ServeCommand::Start => start(args.port, args.role, args.name, args.foreground).await,
        ServeCommand::Stop => stop().await,
        ServeCommand::Status => status().await,
    }
}

async fn start(port: u16, role: ServeRole, name: Option<String>, foreground: bool) -> Result<()> {
    let max_role = match role {
        ServeRole::Observer => ProjectRole::Observer,
        ServeRole::Contributor => ProjectRole::Contributor,
    };
    let name = name.unwrap_or_else(|| {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "orbit".to_string())
    });

    let resp = orbit_client::ipc::send_raw(&Request::StartServing {
        port,
        max_role,
        name: name.clone(),
    })
    .await?;

    match resp {
        Response::ServingStarted {
            port: actual_port,
            observer_token: _,
            contributor_token,
        } => {
            let role_label = if contributor_token.is_some() {
                "contributor"
            } else {
                "observer"
            };
            println!("Serving on port {actual_port} as '{name}' [{role_label}]");
            println!("Run `orbit serve stop` to stop sharing.");

            if foreground {
                tokio::signal::ctrl_c().await?;
                stop().await?;
            }
        }
        Response::Error { message } => anyhow::bail!("serve error: {message}"),
        _ => anyhow::bail!("unexpected response from daemon"),
    }
    Ok(())
}

async fn stop() -> Result<()> {
    let resp = orbit_client::ipc::send_raw(&Request::StopServing).await?;
    match resp {
        Response::ServingStopped => println!("Sharing stopped."),
        Response::Error { message } => anyhow::bail!("stop error: {message}"),
        _ => anyhow::bail!("unexpected response from daemon"),
    }
    Ok(())
}

async fn status() -> Result<()> {
    let resp = orbit_client::ipc::send_raw(&Request::ListNetworkPeers).await?;
    match resp {
        Response::NetworkPeers { peers } => {
            if peers.is_empty() {
                println!("Not sharing.");
                return Ok(());
            }
            println!("{} peer(s) connected:", peers.len());
            println!("{:<25} {:<12} REQUESTS", "ADDRESS", "ROLE");
            println!("{}", "-".repeat(50));
            for p in &peers {
                println!(
                    "{:<25} {:<12} {}",
                    p.addr,
                    format!("{:?}", p.role),
                    p.requests
                );
            }
        }
        Response::Error { message } => println!("Not sharing: {message}"),
        _ => anyhow::bail!("unexpected response from daemon"),
    }
    Ok(())
}
