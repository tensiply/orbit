mod app;
mod mcp;
mod theme;
mod views;
mod widget;

pub use theme::Palette;

use orbit_core::{engine::Engine, jira::TaskContext};

pub struct LaunchParams {
    pub engine: Engine,
    pub workspace: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    pub no_tmux: bool,
    pub task_context: Option<TaskContext>,
}

pub async fn run() -> anyhow::Result<Option<LaunchParams>> {
    app::run().await
}
