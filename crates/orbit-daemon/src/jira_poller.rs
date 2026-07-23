use orbit_core::{
    jira,
    task::{self, UpsertData},
    user_config::UserConfig,
};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub async fn run_poll_loop(interval: Duration, mut shutdown_rx: broadcast::Receiver<()>) {
    let mut ticker = tokio::time::interval(interval);
    // First tick fires immediately so the task store is populated right after daemon start.
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
            info!("jira poll: {} issues fetched", issues.len());
            let workspace = {
                let cfg = UserConfig::load();
                let root = cfg.ai_root_expanded();
                root.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "AI".into())
            };
            let mut upserted = 0usize;
            for issue in &issues {
                let orbit_task = issue.to_orbit_task(&workspace, None, None, None);
                let url = match &orbit_task.source {
                    orbit_core::task::TaskSource::Plugin { url, .. } => url.clone(),
                    _ => None,
                };
                let data = UpsertData {
                    title: orbit_task.title,
                    status: orbit_task.status,
                    priority: orbit_task.priority,
                    task_type: orbit_task.task_type,
                    url,
                    tenant: None,
                    project: None,
                    repository: None,
                };
                match task::upsert_by_external_id(&workspace, "jira", &issue.key, data) {
                    Ok(_) => upserted += 1,
                    Err(e) => warn!("jira upsert failed for {}: {e}", issue.key),
                }
            }
            info!("jira poll: {upserted} tasks upserted into store");
        }
        Err(e) => warn!("jira poll task panicked: {e}"),
    }
}
