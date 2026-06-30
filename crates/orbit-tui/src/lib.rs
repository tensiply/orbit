mod app;
mod mcp;
mod views;
mod widget;

use orbit_core::engine::Engine;

pub struct LaunchParams {
    pub engine: Engine,
    pub workspace: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    pub no_tmux: bool,
}

pub async fn run() -> anyhow::Result<Option<LaunchParams>> {
    app::run().await
}
