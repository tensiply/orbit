use anyhow::Result;
use orbit_cli::{Cli, run};

#[tokio::main]
async fn main() -> Result<()> {
    // Dev build: verbose logging by default
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "orbit=debug,orbit_engine=debug,orbit_daemon=debug".into()),
        )
        .init();

    if std::env::args().any(|a| a == "-h" || a == "--help") {
        orbit_cli::banner::print();
    }
    let cli = Cli::parse_dev();
    run(cli).await
}
