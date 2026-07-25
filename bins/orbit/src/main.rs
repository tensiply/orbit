use anyhow::Result;
use clap::Parser;
use orbit_cli::{Cli, run};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "orbit=info".into()),
        )
        .init();

    if std::env::args().any(|a| a == "-h" || a == "--help") {
        orbit_cli::banner::print();
    }
    let cli = Cli::parse();
    run(cli).await
}
