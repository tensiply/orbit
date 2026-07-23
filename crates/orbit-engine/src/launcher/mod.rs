pub mod agents;
pub mod engine_hooks;
pub mod plugin_hooks;
pub mod render;
pub mod runtime;
pub mod tmux;

use anyhow::{Result, bail};
use orbit_core::{context::OrbitScope, engine::Engine, jira::TaskContext, session::Session};
use std::{fs, io::Write, os::unix::process::CommandExt, path::Path, process::Command};

use crate::config::MergedConfig;

// ── public API ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct LaunchOptions {
    /// Skip tmux wrapping even if tmux is available.
    pub no_tmux: bool,
    /// Force a brand-new tmux session even if one for this scope already exists.
    pub new_session: bool,
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
    task_context: Option<&TaskContext>,
) -> Result<()> {
    // 1. Runtime dirs
    let paths = runtime::setup(scope, engine)?;

    // 2. Agent materialisation
    agents::build(scope, engine, &paths.runtime_dir, &config.instructions, config.commands_filter.as_ref())?;

    // 2b. Plugin context + pre-launch hooks
    let mut config = config.clone();
    let state = orbit_core::plugin::PluginState::load();
    let plugins = orbit_core::plugin::load_all();
    plugin_hooks::inject_context(&state, &plugins, &mut config, &paths.runtime_dir)?;
    for path in plugin_hooks::run_pre_launch(&state, &plugins, &paths.runtime_dir) {
        if !config.instructions.contains(&path) {
            config.instructions.push(path);
        }
    }

    // 2c. Task context injection — fetch full detail (description + comments).
    if let Some(task) = task_context {
        let md = match orbit_core::jira::fetch_issue_detail(&task.key) {
            Ok(detail) => orbit_core::jira::render_task_detail_instructions(&detail),
            Err(_) => orbit_core::jira::render_task_instructions(task),
        };
        let path = paths.runtime_dir.join("task-context.md");
        fs::write(&path, &md)?;
        if !config.instructions.contains(&path) {
            config.instructions.push(path);
        }
    }

    // 2d. Engine hooks settings (Claude only) — write runtime settings file for --settings
    let hooks_settings_path = if engine == Engine::Claude {
        let state = orbit_core::engine_hook::EngineHookState::load();
        let catalog = orbit_core::engine_hook::load_all();
        if let Some(val) = engine_hooks::build_settings(&state, &catalog) {
            let path = paths.runtime_dir.join("claude-hooks-settings.json");
            fs::write(&path, serde_json::to_string_pretty(&val)?)?;
            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    // 3a. For Gemini: write merged instructions as GEMINI.md so includeDirectories picks it up
    if engine == Engine::Gemini {
        let gemini_ctx = paths.runtime_dir.join("GEMINI.md");
        build_gemini_context(&config.instructions, &gemini_ctx)?;
        config.instructions.push(gemini_ctx);
    }

    // 3. Write config file (Gemini: runtime_dir already in instructions above)
    let rendered = render::render(&config, engine);
    fs::write(&paths.config_file, serde_json::to_string_pretty(&rendered)?)?;

    // 3b. For Claude: clean CLAUDE.md @refs already injected by orbit, then write
    // the full instruction set as the system prompt context file.
    let context_file = if engine == Engine::Claude {
        cleanup_claude_md_overlapping_refs(&scope.work_dir, &config.instructions);
        let ctx_path = paths.runtime_dir.join("context.md");
        build_claude_context(&config.instructions, &ctx_path)?;
        Some(ctx_path)
    } else {
        None
    };

    // 4. Decide tmux strategy before registering the session
    let username = orbit_core::user_config::UserConfig::load().user.name;
    let tmux_name = tmux_session_name(scope, engine, &username);
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

    // 6. Sync workspace auth into tenant data dir, then set environment variables
    if let Err(e) = runtime::sync_workspace_auth(&paths) {
        tracing::warn!("could not sync workspace auth: {e}");
    }
    set_env(scope, engine, &paths, &config.env);

    // 7. cd into work_dir, then exec
    std::env::set_current_dir(&scope.work_dir)?;

    let title = window_title(scope, engine);
    set_terminal_title(&title);

    if use_tmux {
        exec_with_tmux(
            engine,
            &paths.config_file,
            context_file.as_deref(),
            hooks_settings_path.as_deref(),
            &tmux_name,
            &title,
            opts.new_session,
        )
    } else {
        exec_engine(
            engine,
            &paths.config_file,
            context_file.as_deref(),
            hooks_settings_path.as_deref(),
        )
    }
}

// ── tmux helpers ──────────────────────────────────────────────────────────────

/// Derive a stable tmux session name from scope + engine.
/// Uses only tmux-safe characters (alphanumerics, `-`).
/// Example: "eloir-orbit-claude-befra-jafraus-ecommerce"
///
/// The workspace name is always included in non-global mode so that sessions
/// across different workspaces with the same tenant name never collide
/// (e.g. ~/BeFra and ~/Tensiply both having an "AI" tenant).
pub fn tmux_session_name(scope: &OrbitScope, engine: Engine, username: &str) -> String {
    let safe = |s: &str| {
        s.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-")
    };
    let mut parts: Vec<String> = Vec::new();
    if !username.is_empty() {
        parts.push(safe(username));
    }
    parts.push("orbit".into());
    parts.push(engine.as_str().to_string());
    if !scope.global_mode {
        let ws = scope
            .workspace_root
            .file_name()
            .map(|n| safe(&n.to_string_lossy()))
            .unwrap_or_default();
        if !ws.is_empty() {
            parts.push(ws);
        }
        for seg in [&scope.tenant, &scope.project, &scope.repository] {
            if !seg.is_empty() {
                parts.push(safe(seg));
            }
        }
    }
    parts.join("-")
}

fn exec_with_tmux(
    engine: Engine,
    config_file: &Path,
    context_file: Option<&Path>,
    hooks_settings: Option<&Path>,
    session_name: &str,
    window_name: &str,
    force_new: bool,
) -> Result<()> {
    // Resolve the final session name — unique suffix when forcing a new session.
    let session_name = if force_new {
        tmux::unique_session_name(session_name)
    } else {
        session_name.to_string()
    };
    let session_name = session_name.as_str();

    if !force_new && tmux::session_exists(session_name) {
        // Session already exists — reattach
        tracing::debug!("reattaching to tmux session {session_name}");
        let err = Command::new("tmux")
            .args(["attach-session", "-t", session_name])
            .exec();
        bail!("failed to attach to tmux session {session_name}: {err}");
    }

    // Build the engine command args for tmux
    let (bin, extra_args) = engine_cmd(engine, config_file, context_file, hooks_settings);

    // Create session detached so we can lock the window name before attaching.
    // Without this, the engine rewrites the window name via OSC sequences on startup.
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session")
        .arg("-d")
        .arg("-s")
        .arg(session_name)
        .arg("-n")
        .arg(window_name);

    // Inject orbit env vars into the tmux SESSION via -e flags.
    // tmux update-environment does not include custom vars like XDG_CONFIG_HOME
    // or OPENCODE_CONFIG, so without -e the engine reads the wrong config.
    for var in &[
        "ORBIT_CONFIG_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_STATE_HOME",
        "AI_ENGINE",
        "AI_WORKSPACE_ROOT",
        "AI_CONTEXT_ROOT",
        "AI_GLOBAL_ROOT",
        "AI_TENANT",
        "AI_PROJECT",
        "AI_REPOSITORY",
        "AI_GLOBAL_MODE",
        "OPENCODE_CONFIG",
        "GEMINI_CLI_HOME",
        "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
    ] {
        if let Ok(val) = std::env::var(var) {
            cmd.arg("-e").arg(format!("{var}={val}"));
        }
    }

    cmd.arg("--").arg(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let status = cmd.status()?;
    if !status.success() {
        bail!("failed to create tmux session '{session_name}'");
    }

    // Prevent the engine from overriding the window name via terminal title OSC sequences.
    Command::new("tmux")
        .args([
            "set-window-option",
            "-t",
            session_name,
            "allow-rename",
            "off",
        ])
        .status()
        .ok();

    let err = Command::new("tmux")
        .args(["attach-session", "-t", session_name])
        .exec();
    bail!("failed to attach to tmux session {session_name}: {err}");
}

// ── direct exec ───────────────────────────────────────────────────────────────

fn exec_engine(
    engine: Engine,
    config_file: &Path,
    context_file: Option<&Path>,
    hooks_settings: Option<&Path>,
) -> Result<()> {
    let (bin, extra_args) = engine_cmd(engine, config_file, context_file, hooks_settings);
    let mut cmd = Command::new(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }
    let err = cmd.exec();
    bail!("failed to exec {}: {}", bin, err);
}

fn engine_cmd(
    engine: Engine,
    config_file: &Path,
    context_file: Option<&Path>,
    hooks_settings: Option<&Path>,
) -> (String, Vec<String>) {
    match engine {
        Engine::Claude => {
            let mut args = vec![
                "--mcp-config".to_string(),
                config_file.to_string_lossy().into_owned(),
            ];
            if let Some(ctx) = context_file {
                args.push("--append-system-prompt-file".to_string());
                args.push(ctx.to_string_lossy().into_owned());
            }
            if let Some(h) = hooks_settings {
                args.push("--settings".to_string());
                args.push(h.to_string_lossy().into_owned());
            }
            ("claude".to_string(), args)
        }
        Engine::Opencode | Engine::Gemini => (engine.as_str().to_string(), vec![]),
    }
}

/// Walk from `work_dir` up to home and remove from every `.claude/CLAUDE.md`
/// any `@/abs/path` line whose target is already in orbit's `instructions` list.
/// Orbit injects those files via `--append-system-prompt-file`, so keeping them
/// in CLAUDE.md would duplicate them in the system prompt.
pub fn cleanup_claude_md_overlapping_refs(work_dir: &Path, instructions: &[std::path::PathBuf]) {
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("/"));

    let injected: std::collections::HashSet<&std::path::PathBuf> = instructions.iter().collect();

    let mut current = work_dir.to_path_buf();
    loop {
        let candidate = current.join(".claude").join("CLAUDE.md");
        if candidate.is_file()
            && let Ok(content) = fs::read_to_string(&candidate)
        {
            let cleaned: String = content
                .lines()
                .filter(|line| {
                    let trimmed = line.trim();
                    if let Some(rest) = trimmed.strip_prefix('@') {
                        let p = rest.trim();
                        if p.starts_with('/') {
                            return !injected.contains(&std::path::PathBuf::from(p));
                        }
                    }
                    true
                })
                .collect::<Vec<_>>()
                .join("\n");
            let cleaned = if cleaned.ends_with('\n') {
                cleaned
            } else {
                cleaned + "\n"
            };
            if cleaned != content {
                if let Err(e) = fs::write(&candidate, &cleaned) {
                    tracing::warn!("could not clean {}: {e}", candidate.display());
                } else {
                    tracing::debug!("cleaned orbit-injected @refs from {}", candidate.display());
                }
            }
        }
        if current == home {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
}

/// Returns, for each CLAUDE.md in the work_dir → home hierarchy, the list of
/// @ref paths that overlap with orbit's instructions. Used by dry-run display.
pub fn find_claude_md_overlapping_refs(
    work_dir: &Path,
    instructions: &[std::path::PathBuf],
) -> Vec<(std::path::PathBuf, Vec<std::path::PathBuf>)> {
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("/"));

    let injected: std::collections::HashSet<&std::path::PathBuf> = instructions.iter().collect();
    let mut result = Vec::new();

    let mut current = work_dir.to_path_buf();
    loop {
        let candidate = current.join(".claude").join("CLAUDE.md");
        if candidate.is_file()
            && let Ok(content) = fs::read_to_string(&candidate)
        {
            let overlaps: Vec<std::path::PathBuf> = content
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    trimmed.strip_prefix('@').and_then(|rest| {
                        let p = rest.trim();
                        if p.starts_with('/') {
                            let pb = std::path::PathBuf::from(p);
                            if injected.contains(&pb) {
                                Some(pb)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                })
                .collect();
            if !overlaps.is_empty() {
                result.push((candidate, overlaps));
            }
        }
        if current == home {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    result
}

fn merge_instructions(instructions: &[std::path::PathBuf]) -> String {
    let mut parts = Vec::with_capacity(instructions.len());
    for path in instructions {
        match fs::read_to_string(path) {
            Ok(content) => {
                parts.push(format!("<!-- {} -->\n\n{}", path.display(), content.trim()));
            }
            Err(e) => {
                tracing::warn!("skipping instruction {}: {e}", path.display());
            }
        }
    }
    parts.join("\n\n---\n\n")
}

/// Concatenate all instruction files into a single markdown document
/// for use as Claude's appended system prompt.
fn build_claude_context(instructions: &[std::path::PathBuf], dest: &Path) -> Result<()> {
    fs::write(dest, merge_instructions(instructions))?;
    Ok(())
}

/// Concatenate all instruction files into GEMINI.md so Gemini's
/// context.includeDirectories picks it up from the runtime dir.
fn build_gemini_context(instructions: &[std::path::PathBuf], dest: &Path) -> Result<()> {
    fs::write(dest, merge_instructions(instructions))?;
    Ok(())
}

// ── environment ───────────────────────────────────────────────────────────────

/// Set the environment variables the engine expects.
///
/// # Safety
/// `set_var` is unsafe in Rust 1.80+ because it is not thread-safe.
/// Safe here: single-threaded, called immediately before exec.
fn set_env(
    scope: &OrbitScope,
    engine: Engine,
    paths: &runtime::RuntimePaths,
    extra_env: &std::collections::HashMap<String, String>,
) {
    unsafe {
        // Preserve the real config dir so orbit commands run inside this session
        // can still find the user config (UserConfig checks ORBIT_CONFIG_HOME first).
        if std::env::var("ORBIT_CONFIG_HOME").is_err() {
            let real = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
                directories::BaseDirs::new()
                    .map(|b| b.home_dir().join(".config"))
                    .unwrap_or_else(|| std::path::PathBuf::from("/.config"))
                    .to_string_lossy()
                    .into_owned()
            });
            std::env::set_var("ORBIT_CONFIG_HOME", real);
        }
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

        // gh CLI uses GH_CONFIG_DIR if set, otherwise falls back to $XDG_CONFIG_HOME/gh.
        // Auth for gh is stored at workspace level (workspace_config_dir/gh). If that
        // exists, point gh there; otherwise fall back to the real global config.
        let gh_workspace = paths.workspace_config_dir.join("gh");
        if gh_workspace.exists() {
            std::env::set_var("GH_CONFIG_DIR", &gh_workspace);
        } else {
            let real_config = std::env::var("ORBIT_CONFIG_HOME").unwrap_or_default();
            let real_gh = std::path::PathBuf::from(&real_config).join("gh");
            if real_gh.exists() {
                std::env::set_var("GH_CONFIG_DIR", real_gh);
            }
        }

        // User-defined env vars from orbit.json "env" key — applied last so they
        // can override any of the above if needed. Values are resolved through
        // the secrets layer ($VAR, env://, file://, keychain://).
        for (k, v) in extra_env {
            std::env::set_var(k, orbit_core::secrets::resolve(v));
        }
    }
}

/// Collect all orbit-managed session environment variables into a vec.
///
/// Used both to populate the tmux subprocess environment (`cmd.env`) and to
/// inject variables directly into the new tmux session via `-e KEY=VALUE` flags.
/// The two paths are both required: `cmd.env` sets them on the tmux CLIENT
/// process, while `-e` sets them inside the new tmux SESSION (opencode's env).
/// Without `-e`, tmux's `update-environment` option does not propagate custom
/// variables like `XDG_CONFIG_HOME` or `OPENCODE_CONFIG` to the session.
fn collect_session_env(
    scope: &OrbitScope,
    engine: Engine,
    paths: &runtime::RuntimePaths,
    extra_env: &std::collections::HashMap<String, String>,
) -> Vec<(String, String)> {
    let real_config_home = std::env::var("ORBIT_CONFIG_HOME")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .unwrap_or_else(|_| {
            directories::BaseDirs::new()
                .map(|b| b.home_dir().join(".config"))
                .unwrap_or_else(|| std::path::PathBuf::from("/.config"))
                .to_string_lossy()
                .into_owned()
        });

    let mut env: Vec<(String, String)> = vec![
        ("ORBIT_CONFIG_HOME".into(), real_config_home.clone()),
        (
            "XDG_CONFIG_HOME".into(),
            paths.xdg_config_home.to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME".into(),
            paths.xdg_data.to_string_lossy().into_owned(),
        ),
        (
            "XDG_CACHE_HOME".into(),
            paths.xdg_cache.to_string_lossy().into_owned(),
        ),
        (
            "XDG_STATE_HOME".into(),
            paths.xdg_state.to_string_lossy().into_owned(),
        ),
        ("AI_ENGINE".into(), engine.as_str().to_string()),
        (
            "AI_WORKSPACE_ROOT".into(),
            scope.workspace_root.to_string_lossy().into_owned(),
        ),
        (
            "AI_CONTEXT_ROOT".into(),
            scope.ai_context_root.to_string_lossy().into_owned(),
        ),
        (
            "AI_GLOBAL_ROOT".into(),
            scope.global_ai_root.to_string_lossy().into_owned(),
        ),
        ("AI_TENANT".into(), scope.tenant.clone()),
        ("AI_PROJECT".into(), scope.project.clone()),
        ("AI_REPOSITORY".into(), scope.repository.clone()),
        (
            "AI_GLOBAL_MODE".into(),
            if scope.global_mode { "1" } else { "0" }.into(),
        ),
    ];

    match engine {
        Engine::Opencode => {
            env.push((
                "OPENCODE_CONFIG".into(),
                paths.config_file.to_string_lossy().into_owned(),
            ));
        }
        Engine::Gemini => {
            env.push((
                "GEMINI_CLI_HOME".into(),
                paths.runtime_dir.to_string_lossy().into_owned(),
            ));
            env.push((
                "GEMINI_CLI_SYSTEM_SETTINGS_PATH".into(),
                paths.config_file.to_string_lossy().into_owned(),
            ));
        }
        Engine::Claude => {}
    }

    // gh: prefer workspace-scoped auth; fall back to global if not configured.
    let gh_workspace = paths.workspace_config_dir.join("gh");
    if gh_workspace.exists() {
        env.push(("GH_CONFIG_DIR".into(), gh_workspace.to_string_lossy().into_owned()));
    } else {
        let real_gh = std::path::PathBuf::from(&real_config_home).join("gh");
        if real_gh.exists() {
            env.push(("GH_CONFIG_DIR".into(), real_gh.to_string_lossy().into_owned()));
        }
    }

    for (k, v) in extra_env {
        env.push((k.clone(), orbit_core::secrets::resolve(v)));
    }

    env
}

// ── daemon-side spawn ─────────────────────────────────────────────────────────

/// Spawn a detached tmux session containing the engine. Returns the registered
/// `Session` on success. Intended for daemon use — does NOT exec() the current
/// process and does NOT call `std::env::set_var`.
/// Spawn the engine as a detached tmux session.
///
/// `session_name` overrides the default computed name — use it for plan nodes
/// so each node gets an isolated session rather than reusing a shared one.
/// When `None`, falls back to the scope-derived name and reuses an existing
/// session if one with that name is already running.
pub fn spawn_background(
    scope: &OrbitScope,
    config: &MergedConfig,
    engine: Engine,
    task_context: Option<&TaskContext>,
    session_name: Option<&str>,
    force_new: bool,
) -> Result<orbit_core::session::Session> {
    // 1. Runtime dirs
    let paths = runtime::setup(scope, engine)?;

    // 2. Agent materialisation
    agents::build(scope, engine, &paths.runtime_dir, &config.instructions, config.commands_filter.as_ref())?;

    // 2b. Plugin context + pre-launch hooks
    let mut config = config.clone();
    let state = orbit_core::plugin::PluginState::load();
    let plugins = orbit_core::plugin::load_all();
    plugin_hooks::inject_context(&state, &plugins, &mut config, &paths.runtime_dir)?;
    for path in plugin_hooks::run_pre_launch(&state, &plugins, &paths.runtime_dir) {
        if !config.instructions.contains(&path) {
            config.instructions.push(path);
        }
    }

    // 2c. Task context injection — fetch full detail (description + comments).
    if let Some(task) = task_context {
        let md = match orbit_core::jira::fetch_issue_detail(&task.key) {
            Ok(detail) => orbit_core::jira::render_task_detail_instructions(&detail),
            Err(_) => orbit_core::jira::render_task_instructions(task),
        };
        let path = paths.runtime_dir.join("task-context.md");
        fs::write(&path, &md)?;
        if !config.instructions.contains(&path) {
            config.instructions.push(path);
        }
    }

    // 2d. Engine hooks settings (Claude only) — write runtime settings file for --settings
    let hooks_settings_path = if engine == Engine::Claude {
        let hook_state = orbit_core::engine_hook::EngineHookState::load();
        let catalog = orbit_core::engine_hook::load_all();
        if let Some(val) = engine_hooks::build_settings(&hook_state, &catalog) {
            let path = paths.runtime_dir.join("claude-hooks-settings.json");
            fs::write(&path, serde_json::to_string_pretty(&val)?)?;
            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    // 3a. For Gemini: write merged instructions as GEMINI.md so includeDirectories picks it up
    if engine == Engine::Gemini {
        let gemini_ctx = paths.runtime_dir.join("GEMINI.md");
        build_gemini_context(&config.instructions, &gemini_ctx)?;
        config.instructions.push(gemini_ctx);
    }

    // 3. Write config file (Gemini: runtime_dir already in instructions above)
    let rendered = render::render(&config, engine);
    fs::write(&paths.config_file, serde_json::to_string_pretty(&rendered)?)?;

    // 3b. For Claude: clean CLAUDE.md @refs already injected by orbit, then write
    // the full instruction set as the system prompt context file.
    let context_file = if engine == Engine::Claude {
        cleanup_claude_md_overlapping_refs(&scope.work_dir, &config.instructions);
        let ctx_path = paths.runtime_dir.join("context.md");
        build_claude_context(&config.instructions, &ctx_path)?;
        Some(ctx_path)
    } else {
        None
    };

    // 4. Tmux session name
    let username = orbit_core::user_config::UserConfig::load().user.name;
    let base_name = session_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| tmux_session_name(scope, engine, &username));
    let tmux_name = if force_new {
        tmux::unique_session_name(&base_name)
    } else {
        base_name.clone()
    };

    // Reuse an existing session only when the caller did not supply an override
    // and a new session was not explicitly requested.
    // Plan-node sessions always get a fresh dedicated session.
    if !force_new && session_name.is_none() && tmux::session_exists(&tmux_name) {
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

    // 5. Sync workspace auth into tenant data dir, then build command
    if let Err(e) = runtime::sync_workspace_auth(&paths) {
        tracing::warn!("could not sync workspace auth: {e}");
    }
    let (bin, extra_args) = engine_cmd(
        engine,
        &paths.config_file,
        context_file.as_deref(),
        hooks_settings_path.as_deref(),
    );
    let session_env = collect_session_env(scope, engine, &paths, &config.env);
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session").arg("-d").arg("-s").arg(&tmux_name);
    for (k, v) in &session_env {
        cmd.arg("-e").arg(format!("{k}={v}"));
    }
    cmd.arg("--").arg(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }
    for (k, v) in &session_env {
        cmd.env(k, v);
    }
    cmd.current_dir(&scope.work_dir);

    let status = cmd.status()?;
    if !status.success() {
        bail!("failed to spawn tmux session '{tmux_name}'");
    }

    // Prevent the engine from overriding the window name via OSC sequences.
    Command::new("tmux")
        .args(["set-window-option", "-t", &tmux_name, "allow-rename", "off"])
        .status()
        .ok();

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

/// Spawn an engine in a dedicated tmux session with an explicit user intent.
///
/// Unlike `spawn_background` (interactive mode), this runs the engine in
/// print / headless mode so the agent processes the intent autonomously
/// and exits when done. Used exclusively by plan-node dispatch.
pub fn spawn_plan_node(
    session_name: &str,
    intent: &str,
    scope: &OrbitScope,
    config: &MergedConfig,
    engine: Engine,
) -> Result<orbit_core::session::Session> {
    // 1. Runtime dirs
    let paths = runtime::setup(scope, engine)?;

    // 2. Agent materialisation
    agents::build(scope, engine, &paths.runtime_dir, &config.instructions, config.commands_filter.as_ref())?;

    // 2b. Plugin context + pre-launch hooks
    let mut config = config.clone();
    let state = orbit_core::plugin::PluginState::load();
    let plugins = orbit_core::plugin::load_all();
    plugin_hooks::inject_context(&state, &plugins, &mut config, &paths.runtime_dir)?;
    for path in plugin_hooks::run_pre_launch(&state, &plugins, &paths.runtime_dir) {
        if !config.instructions.contains(&path) {
            config.instructions.push(path);
        }
    }

    // 3. Write config + context files
    let rendered = render::render(&config, engine);
    fs::write(&paths.config_file, serde_json::to_string_pretty(&rendered)?)?;

    let context_file = if engine == Engine::Claude {
        cleanup_claude_md_overlapping_refs(&scope.work_dir, &config.instructions);
        let ctx_path = paths.runtime_dir.join("context.md");
        build_claude_context(&config.instructions, &ctx_path)?;
        Some(ctx_path)
    } else {
        None
    };

    // 4. Sync workspace auth, then build the headless engine command
    if let Err(e) = runtime::sync_workspace_auth(&paths) {
        tracing::warn!("could not sync workspace auth: {e}");
    }
    let (bin, extra_args) =
        plan_node_cmd(engine, &paths.config_file, context_file.as_deref(), intent);

    // 5. Launch in a dedicated detached tmux session
    let session_env = collect_session_env(scope, engine, &paths, &config.env);
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session").arg("-d").arg("-s").arg(session_name);
    for (k, v) in &session_env {
        cmd.arg("-e").arg(format!("{k}={v}"));
    }
    cmd.arg("--").arg(&bin);
    for arg in &extra_args {
        cmd.arg(arg);
    }
    for (k, v) in &session_env {
        cmd.env(k, v);
    }
    cmd.current_dir(&scope.work_dir);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("failed to create plan-node tmux session '{session_name}'");
    }

    // 6. Register session
    let pid = tmux_pane_pid(session_name).unwrap_or(std::process::id());
    let session = orbit_core::session::Session::new(
        pid,
        engine.as_str(),
        &scope.tenant,
        &scope.project,
        &scope.repository,
        scope.work_dir.clone(),
        scope.global_mode,
        Some(session_name.to_string()),
    );
    if let Err(e) = session.save() {
        tracing::warn!("could not save plan-node session: {e}");
    }

    Ok(session)
}

/// Spawn an external executable as a plan node in a dedicated tmux session.
///
/// Unlike `spawn_plan_node`, this bypasses all engine and MCP setup — it runs
/// `rendered_cmd` directly in the node's `work_dir`. The supervisor's output
/// capture and verify strategies apply unchanged.
///
/// Injects ORBIT_* env vars so the subprocess can inspect its context:
/// `ORBIT_PLAN_ID`, `ORBIT_NODE_ID`, `ORBIT_NODE_LABEL`, `ORBIT_NODE_INTENT`.
pub fn spawn_plugin_executor(
    session_name: &str,
    rendered_cmd: &[String],
    work_dir: &Path,
    orbit_env: &std::collections::HashMap<String, String>,
) -> Result<orbit_core::session::Session> {
    anyhow::ensure!(
        !rendered_cmd.is_empty(),
        "executor command must not be empty"
    );

    let mut cmd = Command::new("tmux");
    cmd.arg("new-session")
        .arg("-d")
        .arg("-s")
        .arg(session_name)
        .arg("--")
        .arg(&rendered_cmd[0]);
    for arg in rendered_cmd.iter().skip(1) {
        cmd.arg(arg);
    }
    for (k, v) in orbit_env {
        cmd.env(k, v);
    }
    cmd.current_dir(work_dir);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("failed to create plugin-executor tmux session '{session_name}'");
    }

    let pid = tmux_pane_pid(session_name).unwrap_or(std::process::id());
    let session = orbit_core::session::Session::new(
        pid,
        "shell",
        "",
        "",
        "",
        work_dir.to_path_buf(),
        false,
        Some(session_name.to_string()),
    );
    if let Err(e) = session.save() {
        tracing::warn!("could not save plugin-executor session: {e}");
    }

    Ok(session)
}

/// Build the headless command for a plan node.
fn plan_node_cmd(
    engine: Engine,
    config_file: &Path,
    context_file: Option<&Path>,
    intent: &str,
) -> (String, Vec<String>) {
    match engine {
        Engine::Claude => {
            let mut args = vec![
                "--mcp-config".to_string(),
                config_file.to_string_lossy().into_owned(),
                "-p".to_string(),
                intent.to_string(),
            ];
            if let Some(ctx) = context_file {
                args.push("--append-system-prompt-file".to_string());
                args.push(ctx.to_string_lossy().into_owned());
            }
            ("claude".to_string(), args)
        }
        Engine::Opencode => (
            "opencode".to_string(),
            vec!["run".to_string(), intent.to_string()],
        ),
        Engine::Gemini => (
            "gemini".to_string(),
            vec!["-p".to_string(), intent.to_string()],
        ),
    }
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

/// Build a human-readable window title.
/// Format: `[orbit][<engine>] - <last_scope> - <workspace>/<parent_scopes>`
/// Example: `[orbit][claude] - orbit - AI/AIDEV/AI-ECOSYSTEM`
fn window_title(scope: &OrbitScope, engine: Engine) -> String {
    if scope.global_mode {
        return format!("[orbit][{}]", engine.as_str());
    }

    let workspace = scope
        .workspace_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Build ordered scope segments: tenant [project [repository]]
    let mut all: Vec<&str> = vec![&scope.tenant];
    if !scope.project.is_empty() {
        all.push(&scope.project);
    }
    if !scope.repository.is_empty() {
        all.push(&scope.repository);
    }

    let last = all.last().copied().unwrap_or("");
    let parent_path: Vec<&str> = all[..all.len().saturating_sub(1)].to_vec();

    let path = if parent_path.is_empty() {
        workspace
    } else {
        format!("{}/{}", workspace, parent_path.join("/"))
    };

    format!("[orbit][{}] - {} - {}", engine.as_str(), last, path)
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
                url: None,
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
            workspace_root: "/home/user/AI".into(),
            tenant: "AIDEV".into(),
            project: "AI-ECOSYSTEM".into(),
            repository: "orbit".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            tmux_session_name(&scope, Engine::Opencode, ""),
            "orbit-opencode-ai-aidev-ai-ecosystem-orbit"
        );
        assert_eq!(
            tmux_session_name(&scope, Engine::Opencode, "eloir"),
            "eloir-orbit-opencode-ai-aidev-ai-ecosystem-orbit"
        );
    }

    #[test]
    fn tmux_session_name_includes_workspace_to_avoid_collision() {
        // ~/BeFra and ~/Tensiply both have an "AI" tenant — names must differ.
        let befra = OrbitScope {
            workspace_root: "/home/user/BeFra".into(),
            tenant: "AI".into(),
            global_mode: false,
            ..Default::default()
        };
        let tensiply = OrbitScope {
            workspace_root: "/home/user/Tensiply".into(),
            tenant: "AI".into(),
            global_mode: false,
            ..Default::default()
        };
        let name_befra = tmux_session_name(&befra, Engine::Claude, "eloir");
        let name_tensiply = tmux_session_name(&tensiply, Engine::Claude, "eloir");
        assert_ne!(name_befra, name_tensiply);
        assert_eq!(name_befra, "eloir-orbit-claude-befra-ai");
        assert_eq!(name_tensiply, "eloir-orbit-claude-tensiply-ai");
    }

    #[test]
    fn tmux_session_name_global() {
        let scope = OrbitScope {
            global_mode: true,
            ..Default::default()
        };
        assert_eq!(
            tmux_session_name(&scope, Engine::Claude, ""),
            "orbit-claude"
        );
        assert_eq!(
            tmux_session_name(&scope, Engine::Claude, "eloir"),
            "eloir-orbit-claude"
        );
    }

    #[test]
    fn window_title_global() {
        let scope = OrbitScope {
            global_mode: true,
            ..Default::default()
        };
        assert_eq!(window_title(&scope, Engine::Claude), "[orbit][claude]");
    }

    #[test]
    fn window_title_full_scope() {
        let scope = OrbitScope {
            workspace_root: "/home/user/AI".into(),
            tenant: "AIDEV".into(),
            project: "AI-ECOSYSTEM".into(),
            repository: "orbit".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            window_title(&scope, Engine::Claude),
            "[orbit][claude] - orbit - AI/AIDEV/AI-ECOSYSTEM"
        );
    }

    #[test]
    fn window_title_project_only() {
        let scope = OrbitScope {
            workspace_root: "/home/user/AI".into(),
            tenant: "AIDEV".into(),
            project: "AI-ECOSYSTEM".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            window_title(&scope, Engine::Opencode),
            "[orbit][opencode] - AI-ECOSYSTEM - AI/AIDEV"
        );
    }

    #[test]
    fn window_title_tenant_only() {
        let scope = OrbitScope {
            workspace_root: "/home/user/AI".into(),
            tenant: "AIDEV".into(),
            global_mode: false,
            ..Default::default()
        };
        assert_eq!(
            window_title(&scope, Engine::Gemini),
            "[orbit][gemini] - AIDEV - AI"
        );
    }
}
