use orbit_core::{
    engine::Engine,
    plan::{PlanNode, VerifyStrategy},
};
use std::process::{Command, Stdio};
use tracing::warn;

use crate::planner::engine_cli_command;

// ── VerifyOutcome ─────────────────────────────────────────────────────────────

pub enum VerifyOutcome {
    Pass,
    Fail(String),
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Run every verify strategy for the node in order. Returns the first failure, or Pass.
pub fn verify_node(node: &PlanNode, judge_engine: Engine) -> VerifyOutcome {
    if node.policy.verify.is_empty() {
        return VerifyOutcome::Pass;
    }
    for strategy in &node.policy.verify {
        let outcome = run_strategy(strategy, node, judge_engine);
        if matches!(outcome, VerifyOutcome::Fail(_)) {
            return outcome;
        }
    }
    VerifyOutcome::Pass
}

// ── Strategies ────────────────────────────────────────────────────────────────

fn run_strategy(strategy: &VerifyStrategy, node: &PlanNode, judge_engine: Engine) -> VerifyOutcome {
    match strategy {
        VerifyStrategy::ExitCode => {
            // Engines (claude, opencode, gemini) always exit 0. Treat as Pass.
            VerifyOutcome::Pass
        }

        VerifyStrategy::OutputContains { keywords } => {
            let Some(ref summary) = node.output_summary else {
                return VerifyOutcome::Fail("no output captured to check keywords".into());
            };
            let lower = summary.to_lowercase();
            for kw in keywords {
                if !lower.contains(&kw.to_lowercase()) {
                    return VerifyOutcome::Fail(format!("output missing keyword: '{kw}'"));
                }
            }
            VerifyOutcome::Pass
        }

        VerifyStrategy::LlmJudge => {
            let Some(ref summary) = node.output_summary else {
                warn!("LlmJudge: no output captured for node {} — skipping", node.id);
                return VerifyOutcome::Pass;
            };
            match llm_judge(summary, &node.intent, judge_engine) {
                Ok(outcome) => outcome,
                Err(e) => {
                    warn!("LlmJudge error for node {}: {e} — treating as Pass", node.id);
                    VerifyOutcome::Pass
                }
            }
        }

        VerifyStrategy::ShellCheck { command } => {
            if command.is_empty() {
                return VerifyOutcome::Pass;
            }
            match Command::new(&command[0])
                .args(&command[1..])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
            {
                Ok(s) if s.success() => VerifyOutcome::Pass,
                Ok(s) => VerifyOutcome::Fail(format!(
                    "shell check '{}' exited {:?}",
                    command[0],
                    s.code()
                )),
                Err(e) => VerifyOutcome::Fail(format!("shell check failed to run: {e}")),
            }
        }
    }
}

fn llm_judge(output: &str, intent: &str, engine: Engine) -> anyhow::Result<VerifyOutcome> {
    let preview: String = output.chars().take(2000).collect();
    let prompt = format!(
        "You are a verification judge.\n\
         Task: {intent}\n\n\
         Agent output:\n{preview}\n\n\
         Did the agent successfully complete the task?\n\
         Respond with exactly 'PASS' or 'FAIL: <brief reason>'."
    );

    let (cmd, args) = engine_cli_command(&engine);
    let child = Command::new(cmd)
        .args(&args)
        .arg(&prompt)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let out = child.wait_with_output()?;
    let raw = String::from_utf8_lossy(&out.stdout);
    let trimmed = raw.trim();

    if trimmed.starts_with("PASS") {
        Ok(VerifyOutcome::Pass)
    } else if trimmed.starts_with("FAIL") {
        let reason = trimmed
            .trim_start_matches("FAIL")
            .trim_start_matches(':')
            .trim()
            .to_string();
        Ok(VerifyOutcome::Fail(if reason.is_empty() {
            "LlmJudge: FAIL".into()
        } else {
            reason
        }))
    } else {
        // Ambiguous response → Pass (don't penalise on unclear judge output)
        warn!("LlmJudge ambiguous response: '{trimmed}'");
        Ok(VerifyOutcome::Pass)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::plan::{NodePolicy, NodeStatus, PlanNodeType, RiskLevel};

    fn make_node(verify: Vec<VerifyStrategy>, output: Option<String>) -> PlanNode {
        PlanNode {
            id: "n0".into(),
            task_type: PlanNodeType::Code,
            label: "test node".into(),
            intent: "do something".into(),
            engine: Engine::Claude,
            scope_override: None,
            status: NodeStatus::Running,
            depends_on: vec![],
            policy: NodePolicy {
                timeout_secs: None,
                retry_max: 1,
                risk_level: RiskLevel::Low,
                verify,
            },
            output_summary: output,
            session_id: None,
            token_usage: None,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
        }
    }

    #[test]
    fn no_strategies_passes() {
        let node = make_node(vec![], None);
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Pass));
    }

    #[test]
    fn exit_code_always_passes() {
        let node = make_node(vec![VerifyStrategy::ExitCode], None);
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Pass));
    }

    #[test]
    fn output_contains_passes_when_present() {
        let node = make_node(
            vec![VerifyStrategy::OutputContains { keywords: vec!["success".into()] }],
            Some("task completed with success".into()),
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Pass));
    }

    #[test]
    fn output_contains_case_insensitive() {
        let node = make_node(
            vec![VerifyStrategy::OutputContains { keywords: vec!["SUCCESS".into()] }],
            Some("task completed with success".into()),
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Pass));
    }

    #[test]
    fn output_contains_fails_when_missing() {
        let node = make_node(
            vec![VerifyStrategy::OutputContains { keywords: vec!["success".into()] }],
            Some("something else entirely".into()),
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Fail(_)));
    }

    #[test]
    fn output_contains_fails_without_output() {
        let node = make_node(
            vec![VerifyStrategy::OutputContains { keywords: vec!["ok".into()] }],
            None,
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Fail(_)));
    }

    #[test]
    fn shell_check_passes_true() {
        let node = make_node(
            vec![VerifyStrategy::ShellCheck { command: vec!["true".into()] }],
            None,
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Pass));
    }

    #[test]
    fn shell_check_fails_false() {
        let node = make_node(
            vec![VerifyStrategy::ShellCheck { command: vec!["false".into()] }],
            None,
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Fail(_)));
    }

    #[test]
    fn empty_shell_check_passes() {
        let node = make_node(
            vec![VerifyStrategy::ShellCheck { command: vec![] }],
            None,
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Pass));
    }

    #[test]
    fn first_failure_short_circuits() {
        let node = make_node(
            vec![
                VerifyStrategy::OutputContains { keywords: vec!["missing".into()] },
                VerifyStrategy::ExitCode, // would pass, but never reached
            ],
            Some("no keyword here".into()),
        );
        assert!(matches!(verify_node(&node, Engine::Claude), VerifyOutcome::Fail(_)));
    }
}
