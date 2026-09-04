use anyhow::Result;
use orbit_core::channel::Channel;

#[tokio::main]
async fn main() -> Result<()> {
    orbit_cli::run_channel(Channel::Stable).await
}
