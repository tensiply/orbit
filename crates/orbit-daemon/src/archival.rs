use orbit_core::{
    plan::{Plan, PlanStatus},
    user_config::UserConfig,
};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::info;

pub async fn run_archival_loop(interval: Duration, mut shutdown_rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(interval) => {
                tick();
            }
        }
    }
}

fn tick() {
    let cfg = UserConfig::load();
    if !cfg.plan_retention.auto_prune_enabled {
        return;
    }

    let cutoff_secs = cfg.plan_retention.auto_prune_days as u64 * 86_400;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let plans = Plan::load_all();
    let prunable: Vec<&Plan> = plans
        .iter()
        .filter(|p| {
            matches!(
                p.status,
                PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Cancelled
            ) && now.saturating_sub(p.created_at) >= cutoff_secs
        })
        .collect();

    if prunable.is_empty() {
        return;
    }

    let mut pruned = 0usize;
    for plan in prunable {
        if cfg.plan_retention.archive_on_prune {
            if let Err(e) = plan.archive() {
                tracing::warn!("archival failed for plan {}: {e}", plan.id);
            } else {
                pruned += 1;
            }
        } else {
            if let Err(e) = plan.delete() {
                tracing::warn!("delete failed for plan {}: {e}", plan.id);
            } else {
                pruned += 1;
            }
        }
    }

    if pruned > 0 {
        let action = if cfg.plan_retention.archive_on_prune {
            "archived"
        } else {
            "deleted"
        };
        info!(
            "auto-prune: {pruned} plan(s) {action} (older than {} days)",
            cfg.plan_retention.auto_prune_days
        );
    }
}
