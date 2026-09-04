use anyhow::Result;
use orbit_core::channel::Channel;

#[tokio::main]
async fn main() -> Result<()> {
    // The Canary channel isolates its home to ~/.orbit-canary, so a canary
    // daemon, socket, and data live beside the stable and dev installs without
    // sharing orbitd.
    orbit_cli::run_channel(Channel::Canary).await
}
