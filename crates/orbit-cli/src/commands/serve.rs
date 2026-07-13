use anyhow::Result;
use clap::Args;
use orbit_core::ipc::{ProjectRole, Request, Response};

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// TCP port to listen on
    #[arg(long, default_value = "7373")]
    pub port: u16,
    /// Maximum role to grant network peers (observer | contributor)
    #[arg(long, value_enum, default_value = "observer")]
    pub role: ServeRole,
    /// Instance name for mDNS (default: hostname)
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ServeRole {
    Observer,
    Contributor,
}

pub async fn run(args: ServeArgs) -> Result<()> {
    let max_role = match args.role {
        ServeRole::Observer => ProjectRole::Observer,
        ServeRole::Contributor => ProjectRole::Contributor,
    };
    let name = args.name.unwrap_or_else(|| {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "orbit".to_string())
    });

    let req = Request::StartServing {
        port: args.port,
        max_role,
        name: name.clone(),
    };
    let resp = orbit_client::ipc::send_raw(&req).await?;

    match resp {
        Response::ServingStarted {
            port,
            observer_token: _,
            contributor_token,
        } => {
            println!("orbit serve: listening on port {port}");
            println!("mDNS: announcing as '{name}'");
            if contributor_token.is_some() {
                println!("Role: contributor (peers can read + approve nodes)");
            } else {
                println!("Role: observer (peers can read plans and stream output)");
            }
            println!("Press Ctrl+C to stop sharing.");

            tokio::signal::ctrl_c().await?;

            let _ = orbit_client::ipc::send_raw(&Request::StopServing).await;
            println!("\norbit serve: stopped.");
        }
        Response::Error { message } => {
            anyhow::bail!("serve error: {message}");
        }
        _ => anyhow::bail!("unexpected response from daemon"),
    }
    Ok(())
}
