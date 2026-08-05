use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};
use tracing::warn;

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    PrePlan,
    PostPlan,
    PreNode,
    PostNode,
    /// Fired after a plan is saved and queued (applies to both manual and scheduled plans).
    OnPlanCreated,
    /// Fired when a scheduled plan is triggered by the scheduler loop.
    OnScheduleFired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanHook {
    pub event: HookEvent,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HooksFile {
    #[serde(default)]
    hooks: Vec<PlanHook>,
}

// ── I/O ───────────────────────────────────────────────────────────────────────

fn hooks_path() -> PathBuf {
    crate::data_paths::orbit_home().join("hooks.toml")
}

pub fn load_hooks() -> Vec<PlanHook> {
    let path = hooks_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    toml::from_str::<HooksFile>(&text)
        .map(|f| f.hooks)
        .unwrap_or_default()
}

// ── execution ─────────────────────────────────────────────────────────────────

/// Run all hooks matching `event`. `env` is a list of (key, value) env vars to set.
pub fn run_hooks(event: &HookEvent, env: &[(&str, &str)]) {
    let hooks = load_hooks();
    let event_name = match event {
        HookEvent::PrePlan => "pre_plan",
        HookEvent::PostPlan => "post_plan",
        HookEvent::PreNode => "pre_node",
        HookEvent::PostNode => "post_node",
        HookEvent::OnPlanCreated => "on_plan_created",
        HookEvent::OnScheduleFired => "on_schedule_fired",
    };

    for hook in hooks.iter().filter(|h| &h.event == event) {
        if hook.command.is_empty() {
            continue;
        }
        let mut cmd = Command::new(&hook.command[0]);
        cmd.args(&hook.command[1..]);
        cmd.env("ORBIT_HOOK_EVENT", event_name);
        for (k, v) in env {
            cmd.env(k, v);
        }
        match cmd.status() {
            Ok(s) if !s.success() => {
                warn!("hook '{}' exited {:?}", hook.command[0], s.code())
            }
            Err(e) => warn!("hook '{}' failed to run: {e}", hook.command[0]),
            _ => {}
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn load_empty_returns_empty() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("ORBIT_HOME", tmp.path().to_str().unwrap());
        }
        let hooks = load_hooks();
        unsafe { std::env::remove_var("ORBIT_HOME") };
        assert!(hooks.is_empty());
    }

    #[test]
    fn load_parses_hooks() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let mut f = std::fs::File::create(tmp.path().join("hooks.toml")).unwrap();
        writeln!(
            f,
            "[[hooks]]\nevent = \"pre_plan\"\ncommand = [\"echo\", \"hello\"]"
        )
        .unwrap();
        unsafe {
            std::env::set_var("ORBIT_HOME", tmp.path().to_str().unwrap());
        }
        let hooks = load_hooks();
        unsafe { std::env::remove_var("ORBIT_HOME") };
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, HookEvent::PrePlan);
    }
}
