pub mod agents;
pub mod render;
pub mod runtime;
pub mod tmux;

use anyhow::{Result, bail};
use orbit_core::{context::OrbitScope, engine::Engine, session::Session};
use std::{fs, io::Write, os::unix::process::CommandExt, path::Path, process::Command};

use crate::config::MergedConfig;

// ── public API ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct LaunchOptions {
    /// Skip tmux wrapping even if tmux is available.
    pub no_tmux: bool,
}

/// Full launch sequence:
/// 1. Create runtime directory structure
/// 2. Materialise agent/command files for the engine
/// 3. Render and write the engine config file
/// 4. Register session (before env override)
/// 5. Set environment variables
/// 6. `exec` into tmux (wrapping the engine) or directly into the engine
pub fn launch(
    scope: &OrbitScope,
    config: &MergedConfig,
    engine: Engine,
    opts: LaunchOptions,
) -> Result<()> {
    // 1. Runtime dirs
    let paths = runtime::setup(scope, engine)?;

    // 2. Agent materialisation
    agents::build(scope, engine, &paths.runtime_dir, &config.instructions)?;

    // 3. Write config file
    let rendered = render::render(config, engine);
    fs::write(&paths.config_file, serde_json::to_string_pretty(&rendered)?)?;

    // 4. Decide tmux strategy before registering the session
    let tmux_name = tmux_session_name(scope, engine);
    let use_tmux = !opts.no_tmux && !tmux::already_inside() && tmux::ensure_available(); // prompts to install if missing + TTY

    // 5. Register session — BEFORE set_env() overwrites XDG_DATA_HOME
    let session = Session::new(
        std::process::id(),
        engine.as_str(),
        &scope.tenant,
        &scope.project,
        &scope.repository,
        scope.work_dir.clone(),
        scope.global_mode,
        if use_tmux {
            Some(tmux_name.clone())
        } else {
            None
        },
    );
    if let Err(e) = session.save() {
        tracing::warn!("could not save session: {e}");
    }

    // 6. Set environment variables
    set_env(scope, engine, &paths);

    // 7. cd into work_dir, then exec
    std::env::set_current_dir(&scope.work_dir)?;

    let title = window_title(scope, engine);
    set_terminal_title(&title);

    if use_tmux {
        exec_with_tmux(engine, &paths.config_file, &tmux_name, &title)
    } else {
        exec_engine(engine, &paths.config_file)
    }
}

// ── tmux helpers ──────────────────────────────────────────────────────────────

/// Derive a stable tmux session name from scope + engine.
/// Example: "orbit-opencode-aidev-ai-ecosystem-orbit"
pub fn tmux_session_name(scope: &OrbitScope, engine: Engine) -> String {
    let parts: Vec<String> = if scope.global_mode {
        vec![engine.as_str().to_string()]
    } else {
        let mut p = vec![engine.as_str().to_string()];
        for seg in [&scope.tenant, &scope.project, &scope.repository] {
            if !seg.is_empty() {
                p.push(seg.to_lowercase());
            }
        }
        p
    };
    format!("orbit-{}", parts.join("-"))
}

fn exec_with_tmux(
    engine: Engine,
    config_file: &Path,
    session_name: &str,
    window_name: &str,
) -> Result<()> {
    if tmux::session_exists(session_name) {
        // Session already exists — reattach
        tracing::debug!("reattaching to tmux session {session_name}");
        let err = Command::new("tmux")
            .args(["attach-session", "-t", session_name])
            .exec();
        bail!("failed to attach to tmux session {session_name}: {err}");
    }

    // Build the engine command args for tmux
    let (bin, extra_args) = engine_cmd(engine, config_file);

    // tmux new-session -s <name> -n <window> -- <bin> [args...]
    // Env vars already set in process environment — tmux inherits them.
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session")
        .arg("-s")
        .arg(session_name)
        .arg("-n")
        .arg(window_name)
        .arg("--")
        .arg(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let err = cmd.exec();
    bail!("failed to exec tmux new-session: {err}");
}

// ── direct exec ───────────────────────────────────────────────────────────────

fn exec_engine(engine: Engine, config_file: &Path) -> Result<()> {
    let (bin, extra_args) = engine_cmd(engine, config_file);
    let mut cmd = Command::new(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }
    let err = cmd.exec();
    bail!("failed to exec {}: {}", bin, err);
}

fn engine_cmd(engine: Engine, config_file: &Path) -> (String, Vec<String>) {
    match engine {
        Engine::Claude => (
            "claude".to_string(),
            vec![
                "--mcp-config".to_string(),
                config_file.to_string_lossy().into_owned(),
            ],
        ),
        Engine::Opencode | Engine::Gemini => (engine.as_str().to_string(), vec![]),
    }
}

// ── environment ───────────────────────────────────────────────────────────────

/// Set the environment variables the engine expects.
///
/// # Safety
/// `set_var` is unsafe in Rust 1.80+ because it is not thread-safe.
/// Safe here: single-threaded, called immediately before exec.
fn set_env(scope: &OrbitScope, engine: Engine, paths: &runtime::RuntimePaths) {
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &paths.xdg_config_home);
        std::env::set_var("XDG_DATA_HOME", &paths.xdg_data);
        std::env::set_var("XDG_CACHE_HOME", &paths.xdg_cache);
        std::env::set_var("XDG_STATE_HOME", &paths.xdg_state);

        std::env::set_var("AI_ENGINE", engine.as_str());
        std::env::set_var(
            "AI_WORKSPACE_ROOT",
            scope.workspace_root.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "AI_CONTEXT_ROOT",
            scope.ai_context_root.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "AI_GLOBAL_ROOT",
            scope.global_ai_root.to_string_lossy().as_ref(),
        );
        std::env::set_var("AI_TENANT", &scope.tenant);
        std::env::set_var("AI_PROJECT", &scope.project);
        std::env::set_var("AI_REPOSITORY", &scope.repository);
        std::env::set_var("AI_GLOBAL_MODE", if scope.global_mode { "1" } else { "0" });

        match engine {
            Engine::Opencode => {
                std::env::set_var("OPENCODE_CONFIG", &paths.config_file);
            }
            Engine::Gemini => {
                std::env::set_var("GEMINI_CLI_HOME", &paths.runtime_dir);
                std::env::set_var("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &paths.config_file);
            }
            Engine::Claude => {}
        }
    }
}

/// Apply engine env vars to a `Command` instead of the current process.
/// Used by the daemon to avoid polluting its own environment.
fn apply_env_to_cmd(
    cmd: &mut Command,
    scope: &OrbitScope,
    engine: Engine,
    paths: &runtime::RuntimePaths,
) {
    cmd.env("XDG_CONFIG_HOME", &paths.xdg_config_home)
        .env("XDG_DATA_HOME", &paths.xdg_data)
        .env("XDG_CACHE_HOME", &paths.xdg_cache)
        .env("XDG_STATE_HOME", &paths.xdg_state)
        .env("AI_ENGINE", engine.as_str())
        .env("AI_WORKSPACE_ROOT", &scope.workspace_root)
        .env("AI_CONTEXT_ROOT", &scope.ai_context_root)
        .env("AI_GLOBAL_ROOT", &scope.global_ai_root)
        .env("AI_TENANT", &scope.tenant)
        .env("AI_PROJECT", &scope.project)
        .env("AI_REPOSITORY", &scope.repository)
        .env("AI_GLOBAL_MODE", if scope.global_mode { "1" } else { "0" });

    match engine {
        Engine::Opencode => {
            cmd.env("OPENCODE_CONFIG", &paths.config_file);
        }
        Engine::Gemini => {
            cmd.env("GEMINI_CLI_HOME", &paths.runtime_dir)
                .env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &paths.config_file);
        }
        Engine::Claude => {}
    }
}

// ── daemon-side spawn ─────────────────────────────────────────────────────────

/// Spawn a detached tmux session containing the engine. Returns the registered
/// `Session` on success. Intended for daemon use — does NOT exec() the current
/// process and does NOT call `std::env::set_var`.
pub fn spawn_background(
    scope: &OrbitScope,
    config: &MergedConfig,
    engine: Engine,
) -> Result<orbit_core::session::Session> {
    // 1. Runtime dirs
    let paths = runtime::setup(scope, engine)?;

    // 2. Agent materialisation
    agents::build(scope, engine, &paths.runtime_dir, &config.instructions)?;

    // 3. Write config file
    let rendered = render::render(config, engine);
    fs::write(&paths.config_file, serde_json::to_string_pretty(&rendered)?)?;

    // 4. Tmux session name
    let tmux_name = tmux_session_name(scope, engine);

    if tmux::session_exists(&tmux_name) {
        // Already running — return the existing session name so client can attach
        let pid = tmux_pane_pid(&tmux_name).unwrap_or(std::process::id());
        let session = orbit_core::session::Session::new(
            pid,
            engine.as_str(),
            &scope.tenant,
            &scope.project,
            &scope.repository,
            scope.work_dir.clone(),
            scope.global_mode,
            Some(tmux_name),
        );
        return Ok(session);
    }

    // 5. Build command
    let (bin, extra_args) = engine_cmd(engine, &paths.config_file);
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session")
        .arg("-d")
        .arg("-s")
        .arg(&tmux_name)
        .arg("--")
        .arg(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }
    apply_env_to_cmd(&mut cmd, scope, engine, &paths);
    cmd.current_dir(&scope.work_dir);

    let status = cmd.status()?;
    if !status.success() {
        bail!("failed to spawn tmux session '{tmux_name}'");
    }

    // 6. Get pane PID
    let pid = tmux_pane_pid(&tmux_name).unwrap_or(std::process::id());

    // 7. Register session
    let session = orbit_core::session::Session::new(
        pid,
        engine.as_str(),
        &scope.tenant,
        &scope.project,
        &scope.repository,
        scope.work_dir.clone(),
        scope.global_mode,
        Some(tmux_name),
    );
    if let Err(e) = session.save() {
        tracing::warn!("could not save session: {e}");
    }

    Ok(session)
}

fn tmux_pane_pid(session_name: &str) -> Option<u32> {
    let out = Command::new("tmux")
        .args(["list-panes", "-t", session_name, "-F", "#{pane_pid}"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout)
        .ok()?
        .trim()
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

// ── terminal title ────────────────────────────────────────────────────────────

/// Build a human-readable title: `orbit · <engine> · <tenant>/<project>/<repo>`.
fn window_title(scope: &OrbitScope, engine: Engine) -> String {
    if scope.global_mode {
        format!("orbit · {}", engine.as_str())
    } else {
        let mut segments: Vec<&str> = vec![&scope.tenant];
        if !scope.project.is_empty() {
            segments.push(&scope.project);
        }
        if !scope.repository.is_empty() {
            segments.push(&scope.repository);
        }
        format!("orbit · {} · {}", engine.as_str(), segments.join("/"))
    }
}

/// Emit an xterm OSC escape to set the terminal window/tab title.
/// No-ops when stdout is not a TTY (CI, pipes).
fn set_terminal_title(title: &str) {
    use std::io::IsTerminal;
    if std::io::stdout().is_terminal() {
        print!("\x1b]0;{title}\x07");
        let _ = std::io::stdout().flush();
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpServer, MergedConfig};
    use orbit_core::context::OrbitScope;
    use std::{collections::HashMap, fs, path::PathBuf};
    use tempfile::TempDir;

    fn minimal_scope(tmp: &TempDir) -> OrbitScope {
        let root = tmp.path().to_path_buf();
        OrbitScope {
            workspace_root: root.clone(),
            ai_context_root: root.clone(),
            global_ai_root: root.clone(),
            tenant_dir: root.clone(),
            code_root: root.clone(),
            work_dir: root.clone(),
            global_mode: true,
            ..Default::default()
        }
    }

    fn config_with_mcp() -> MergedConfig {
        let mut cfg = MergedConfig::default();
        cfg.mcp.insert(
            "my-server".into(),
            McpServer {
                command: vec!["npx".into(), "-y".into(), "some-mcp".into()],
                environment: HashMap::from([("KEY".into(), "val".into())]),
                cwd: None,
                server_type: "local".into(),
            },
        );
        cfg.instructions.push(PathBuf::from("/fake/README.md"));
        cfg
    }

    #[test]
    fn setup_creates_runtime_dirs() {
        let tmp = TempDir::new().unwrap();
        let scope = minimal_scope(&tmp);
        let paths = runtime::setup(&scope, Engine::Opencode).unwrap();
        assert!(paths.xdg_data.is_dir());
        assert!(paths.xdg_cache.is_dir());
        assert!(paths.xdg_state.is_dir());
    }

    #[test]
    fn writes_opencode_config_file() {
        let tmp = TempDir::new().unwrap();
        let scope = minimal_scope(&tmp);
        let cfg = config_with_mcp();
        let paths = runtime::setup(&scope, Engine::Opencode).unwrap();
        let rendered = render::render(&cfg, Engine::Opencode);
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&rendered).unwrap(),
        )
        .unwrap();
        assert!(paths.config_file.exists());
        let content = fs::read_to_string(&paths.config_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcp"]["my-server"]["command"].is_array());
    }

    #[test]
    fn writes_claude_mcp_config() {
        let tmp = TempDir::new().unwrap();
        let scope = minimal_scope(&tmp);
        let cfg = config_with_mcp();
        let paths = runtime::setup(&scope, Engine::Claude).unwrap();
        let rendered = render::render(&cfg, Engine::Claude);
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&rendered).unwrap(),
        )
        .unwrap();
        let content = fs::read_to_string(&paths.config_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.as_object().unwrap().len(), 1);
        assert!(parsed["mcpServers"].is_object());
    }

    #[test]
    fn runtime_paths_differ_per_engine() {
        let tmp = TempDir::new().unwrap();
        let scope = minimal_scope(&tmp);
        let oc = runtime::setup(&scope, Engine::Opencode).unwrap();
        let cl = runtime::setup(&scope, Engine::Claude).unwrap();
        assert_ne!(oc.runtime_dir, cl.runtime_dir);
    }

    #[test]
    fn tmux_session_name_full_scope() {
        let scope = OrbitScope {
            tenant: "AIDEV".into(),
            project: "AI-ECOSYSTEM".into(),
            repository: "orbit".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            tmux_session_name(&scope, Engine::Opencode),
            "orbit-opencode-aidev-ai-ecosystem-orbit"
        );
    }

    #[test]
    fn tmux_session_name_global() {
        let scope = OrbitScope {
            global_mode: true,
            ..Default::default()
        };
        assert_eq!(tmux_session_name(&scope, Engine::Claude), "orbit-claude");
    }

    #[test]
    fn window_title_global() {
        let scope = OrbitScope {
            global_mode: true,
            ..Default::default()
        };
        assert_eq!(window_title(&scope, Engine::Claude), "orbit · claude");
    }

    #[test]
    fn window_title_full_scope() {
        let scope = OrbitScope {
            tenant: "AIDEV".into(),
            project: "AI-ECOSYSTEM".into(),
            repository: "orbit".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            window_title(&scope, Engine::Opencode),
            "orbit · opencode · AIDEV/AI-ECOSYSTEM/orbit"
        );
    }

    #[test]
    fn window_title_tenant_only() {
        let scope = OrbitScope {
            tenant: "AIDEV".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            window_title(&scope, Engine::Gemini),
            "orbit · gemini · AIDEV"
        );
    }
}
