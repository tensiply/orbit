use orbit_core::{
    audit::{AuditEvent, append_event},
    hooks::{HookEvent, run_hooks},
    memory::{find_similar, load_recent_runs},
    plan::{CrossRepoSpec, PlanScope, PlanStatus},
    schedule::{ScheduleKind, load_all, next_cron_after, now_secs, upsert},
};
use orbit_planner::{
    backend::CliBackend,
    planner::{PlannerConfig, invoke_planner},
};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub async fn run_scheduler_loop(interval: Duration, mut shutdown_rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(interval) => {
                tick_schedules();
            }
        }
    }
}

fn tick_schedules() {
    let now = now_secs();
    let schedules = load_all();

    for sched in schedules {
        let Some(next) = sched.next_run else {
            continue; // exhausted Once schedule
        };
        if next > now {
            continue;
        }

        info!(
            "schedule {} due — firing '{}'",
            sched.id,
            &sched.intent.chars().take(60).collect::<String>()
        );
        fire_schedule(&sched, now);
    }
}

fn fire_schedule(sched: &orbit_core::schedule::ScheduledPlan, now: u64) {
    let scope = PlanScope {
        workspace: sched.workspace.clone(),
        tenant: sched.tenant.clone(),
        project: sched.project.clone(),
        repository: sched.repository.clone(),
    };

    let extra_repos: Vec<CrossRepoSpec> = sched
        .repos
        .iter()
        .map(|p| CrossRepoSpec {
            alias: p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            workspace: None,
            tenant: None,
            project: None,
            repository: Some(p.to_string_lossy().to_string()),
        })
        .collect();

    let recent = {
        let similar = find_similar(&sched.intent, 5);
        if similar.is_empty() {
            load_recent_runs(3)
        } else {
            similar
        }
    };
    let cfg = PlannerConfig::default();

    match invoke_planner(
        &sched.intent,
        &scope,
        &recent,
        &cfg,
        &CliBackend::new(cfg.engine),
        &extra_repos,
    ) {
        Err(e) => {
            warn!("scheduler: planner failed for schedule {}: {e}", sched.id);
        }
        Ok((mut plan, _)) => {
            plan.status = PlanStatus::Running;
            let plan_id = plan.id.clone();
            let _ = append_event(&AuditEvent::PlanCreated {
                plan_id: plan_id.clone(),
                intent: sched.intent.clone(),
                node_count: plan.nodes.len(),
                timestamp: now,
            });
            run_hooks(
                &HookEvent::OnScheduleFired,
                &[
                    ("ORBIT_SCHEDULE_ID", &sched.id),
                    ("ORBIT_PLAN_ID", &plan_id),
                    ("ORBIT_PLAN_INTENT", &sched.intent),
                ],
            );
            if let Err(e) = plan.save() {
                warn!("scheduler: failed to save plan {plan_id}: {e}");
                return;
            }
            run_hooks(
                &HookEvent::OnPlanCreated,
                &[
                    ("ORBIT_PLAN_ID", &plan_id),
                    ("ORBIT_PLAN_INTENT", &sched.intent),
                ],
            );
            info!(
                "scheduler: created plan {plan_id} for schedule {}",
                sched.id
            );

            let mut updated = sched.clone();
            updated.last_run = Some(now);
            updated.run_count += 1;
            updated.next_run = match &sched.schedule {
                ScheduleKind::Once { .. } => None, // exhausted
                ScheduleKind::Cron { expr } => next_cron_after(expr, now).unwrap_or(None),
            };
            if let Err(e) = upsert(updated) {
                warn!("scheduler: failed to update schedule {}: {e}", sched.id);
            }
        }
    }
}
