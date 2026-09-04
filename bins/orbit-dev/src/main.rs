use anyhow::Result;
use orbit_core::channel::Channel;

#[tokio::main]
async fn main() -> Result<()> {
    // The Dev channel isolates its home to ~/.orbit-dev (derived from the
    // channel), so a dev daemon, socket, and data live beside the stable
    // install without sharing orbitd.
    orbit_cli::run_channel(Channel::Dev).await
}
