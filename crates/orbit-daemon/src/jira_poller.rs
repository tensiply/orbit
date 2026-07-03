use orbit_core::jira;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub async fn run_poll_loop(interval: Duration, mut shutdown_rx: broadcast::Receiver<()>) {
    let mut ticker = tokio::time::interval(interval);
    // First tick fires immediately so cache is populated right after daemon start.
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                poll_once().await;
            }
            _ = shutdown_rx.recv() => break,
        }
    }
}

async fn poll_once() {
    let result = tokio::task::spawn_blocking(|| {
        let orgs = jira::load_orgs();
        if orgs.is_empty() {
            return vec![];
        }
        jira::fetch_issues(&orgs)
    })
    .await;

    match result {
        Ok(issues) => {
            info!("jira cache updated: {} issues", issues.len());
            jira::write_issues_cache(&issues);
        }
        Err(e) => warn!("jira poll task panicked: {e}"),
    }
}
