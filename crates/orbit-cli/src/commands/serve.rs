use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::ipc::{ProjectRole, Request, Response};

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[command(subcommand)]
    pub command: Option<ServeCommand>,

    /// TCP port to listen on (used when no subcommand is given)
    #[arg(long, default_value = "7373", global = true)]
    pub port: u16,

    /// Maximum role to grant network peers (used when no subcommand is given)
    #[arg(long, value_enum, default_value = "observer", global = true)]
    pub role: ServeRole,

    /// Instance name for mDNS (default: hostname)
    #[arg(long, global = true)]
    pub name: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ServeCommand {
    /// Start sharing (default when no subcommand is given)
    Start,
    /// Stop sharing and close the TCP listener
    Stop,
    /// Show current sharing status and connected peers
    Status,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ServeRole {
    Observer,
    Contributor,
}

pub async fn run(args: ServeArgs) -> Result<()> {
    match args.command.unwrap_or(ServeCommand::Start) {
        ServeCommand::Start => start(args.port, args.role, args.name).await,
        ServeCommand::Stop => stop().await,
        ServeCommand::Status => status().await,
    }
}

async fn start(port: u16, role: ServeRole, name: Option<String>) -> Result<()> {
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

    let req = Request::StartServing {
        port,
        max_role,
        name: name.clone(),
    };
    let resp = orbit_client::ipc::send_raw(&req).await?;

    match resp {
        Response::ServingStarted {
            port: actual_port,
            observer_token: _,
            contributor_token,
        } => {
            println!("orbit serve: listening on port {actual_port}");
            println!("mDNS: announcing as '{name}'");
            if contributor_token.is_some() {
                println!("Role: contributor (peers can read + approve nodes)");
            } else {
                println!("Role: observer (peers can read plans and stream output)");
            }
            println!("Press Ctrl+C or run `orbit serve stop` to stop sharing.");

            tokio::signal::ctrl_c().await?;
            stop().await?;
        }
        Response::Error { message } => {
            anyhow::bail!("serve error: {message}");
        }
        _ => anyhow::bail!("unexpected response from daemon"),
    }
    Ok(())
}

async fn stop() -> Result<()> {
    let resp = orbit_client::ipc::send_raw(&Request::StopServing).await?;
    match resp {
        Response::ServingStopped => println!("orbit serve: stopped."),
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
                println!("Not sharing (no active TCP bridge).");
                return Ok(());
            }
            println!("{} peer(s) connected:", peers.len());
            println!("{:<25} {:<12} {}", "ADDRESS", "ROLE", "REQUESTS");
            println!("{}", "-".repeat(50));
            for p in &peers {
                println!("{:<25} {:<12} {}", p.addr, format!("{:?}", p.role), p.requests);
            }
        }
        Response::Error { message } => {
            // daemon returns Error if not serving — treat as "not sharing"
            println!("Not sharing: {message}");
        }
        _ => anyhow::bail!("unexpected response from daemon"),
    }
    Ok(())
}
