use anyhow::Result;
use orbit_cli::{Cli, run};

#[tokio::main]
async fn main() -> Result<()> {
    // Dev build: isolate from the stable install so a dev daemon, socket, and
    // data live under ~/.orbit-dev instead of ~/.orbit. This lets the release
    // binaries and the dev binaries run side by side without sharing orbitd.
    // Respect an explicit ORBIT_HOME (tests / CI) if the caller set one.
    if std::env::var_os("ORBIT_HOME").is_none()
        && let Some(home) = std::env::var_os("HOME")
    {
        let dev_home = std::path::Path::new(&home).join(".orbit-dev");
        // Safety: set before any orbit code reads ORBIT_HOME and before worker
        // threads run user code — no concurrent env access at this point.
        unsafe { std::env::set_var("ORBIT_HOME", dev_home) };
    }

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
