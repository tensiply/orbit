use anyhow::Result;
use orbit_core::engine::Engine;
use std::process::{Command, Stdio};

use crate::planner::engine_cli_command;

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Abstraction over anything that can respond to a text prompt.
/// Implemented by [`CliBackend`] (subprocess) and [`MockBackend`] (tests).
pub trait PlannerBackend: Send + Sync {
    fn call(&self, prompt: &str) -> Result<String>;
}

// ── CliBackend ────────────────────────────────────────────────────────────────

/// Invokes the engine CLI subprocess (`claude -p`, `opencode run`, `gemini -p`).
pub struct CliBackend {
    pub engine: Engine,
}

impl CliBackend {
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }
}

impl PlannerBackend for CliBackend {
    fn call(&self, prompt: &str) -> Result<String> {
        let (cmd, args) = engine_cli_command(&self.engine);
        let child = Command::new(cmd)
            .args(&args)
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn engine CLI `{cmd}`: {e}"))?;

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("engine CLI exited with error: {stderr}");
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

// ── MockBackend ───────────────────────────────────────────────────────────────

/// Returns a fixed response. Use in tests to exercise the full planner/verifier
/// flow without spawning an engine subprocess.
pub struct MockBackend {
    pub response: String,
}

impl MockBackend {
    pub fn new(response: impl Into<String>) -> Self {
        Self { response: response.into() }
    }
}

impl PlannerBackend for MockBackend {
    fn call(&self, _prompt: &str) -> Result<String> {
        Ok(self.response.clone())
    }
}

// ── FailingBackend ────────────────────────────────────────────────────────────

/// Always returns an error. Use in tests for error-path coverage.
pub struct FailingBackend {
    pub message: String,
}

impl FailingBackend {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl PlannerBackend for FailingBackend {
    fn call(&self, _prompt: &str) -> Result<String> {
        anyhow::bail!("{}", self.message)
    }
}
