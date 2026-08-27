use anyhow::Result;
use orbit_core::{engine::Engine, session::Session};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchScope {
    pub workspace: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
    pub engine: String,
    pub new_session: bool,
}

#[derive(Debug, Serialize)]
pub struct LaunchedInfo {
    pub session_id: String,
    pub tmux_name: String,
}

#[tauri::command]
pub async fn session_list() -> Result<Vec<Session>, String> {
    orbit_client::ipc::list_sessions()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn session_kill(id: String) -> Result<(), String> {
    orbit_client::ipc::kill_session(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn session_clean() -> Result<usize, String> {
    orbit_client::ipc::clean_sessions()
        .await
        .map_err(|e| e.to_string())
}

/// Read the Claude Code `ai-title` for the session whose working directory is `work_dir`.
/// Maps work_dir → `~/.claude/projects/<hash>/` and scans JSONL files by most recently modified.
#[tauri::command]
pub async fn get_session_title(work_dir: String, started_at: u64) -> Option<String> {
    let _ = started_at; // kept in signature for API compat; mtime filter removed (timing is unreliable)
    let home = std::env::var("HOME").ok()?;
    // Claude hashes the cwd by replacing '/' with '-' (keeping the leading '-')
    let hash = work_dir.replace('/', "-");
    let projects_dir = std::path::PathBuf::from(&home)
        .join(".claude/projects")
        .join(&hash);
    if !projects_dir.exists() {
        return None;
    }

    let mut candidates: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(&projects_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let mtime = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            Some((mtime, e.path()))
        })
        .collect();

    // Most recently modified first → most likely current session
    candidates.sort_by_key(|c| std::cmp::Reverse(c.0));

    // ai-title is written asynchronously after the first response — scan the full file
    // and keep the last occurrence (Claude Code updates it as conversation continues).
    for (_, path) in candidates {
        if let Ok(f) = std::fs::File::open(&path) {
            let mut last_title: Option<String> = None;
            for line in BufReader::new(f).lines().map_while(Result::ok) {
                if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                    if obj.get("type").and_then(|t| t.as_str()) == Some("ai-title") {
                        if let Some(title) = obj.get("aiTitle").and_then(|t| t.as_str()) {
                            last_title = Some(title.to_string());
                        }
                    }
                }
            }
            if last_title.is_some() {
                return last_title;
            }
        }
    }
    None
}

#[tauri::command]
pub async fn session_launch(scope: LaunchScope) -> Result<LaunchedInfo, String> {
    let launched = orbit_client::ipc::launch_session(
        scope.workspace,
        scope.tenant,
        scope.project,
        scope.repository,
        &scope.engine,
        false,
        scope.new_session,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(LaunchedInfo {
        session_id: launched.session_id,
        tmux_name: launched.tmux_name,
    })
}

// ── Harness inspection ────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HarnessLayer {
    pub path: String,
    pub exists: bool,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct HarnessMcpServer {
    pub name: String,
    pub command: Vec<String>,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct HarnessInstruction {
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Serialize)]
pub struct HarnessEnvVar {
    pub key: String,
    pub value: String,
    pub redacted: bool,
}

#[derive(Debug, Serialize)]
pub struct HarnessCommand {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct HarnessHook {
    pub name: String,
    pub events: String,
}

#[derive(Debug, Serialize)]
pub struct HarnessPluginContext {
    pub name: String,
    /// Truncated preview of the prompt injected as context (if any).
    pub prompt_preview: Option<String>,
    /// Explicit instruction file paths this plugin adds.
    pub instruction_files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HarnessScopeInfo {
    pub workspace: String,
    pub workspace_root: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    pub engine: String,
    pub work_dir: String,
    pub exec_cmd: String,
    pub auth_status: String,
}

#[derive(Debug, Serialize)]
pub struct HarnessReport {
    pub scope: HarnessScopeInfo,
    pub config_layers: Vec<HarnessLayer>,
    pub mcp_layers: Vec<HarnessLayer>,
    pub agent_overlay_dirs: Vec<HarnessLayer>,
    pub instructions: Vec<HarnessInstruction>,
    pub mcp_servers: Vec<HarnessMcpServer>,
    pub env_vars: Vec<HarnessEnvVar>,
    pub commands: Vec<HarnessCommand>,
    pub engine_hooks: Vec<HarnessHook>,
    /// Plugins that will inject context at launch time (dynamic — not in config layers).
    pub plugin_context: Vec<HarnessPluginContext>,
    /// Recent activity entries that will be injected at launch (preview).
    pub activity_preview: Vec<String>,
}

fn shorten_home(home: &Path, p: &Path) -> String {
    if let Ok(rel) = p.strip_prefix(home) {
        format!("~/{}", rel.display())
    } else {
        p.to_string_lossy().into_owned()
    }
}

#[tauri::command]
pub async fn session_harness(
    workspace: Option<String>,
    tenant: Option<String>,
    project: Option<String>,
    repository: Option<String>,
    engine: String,
) -> Result<HarnessReport, String> {
    let eng: Engine = engine.parse().map_err(|_| format!("unknown engine: {engine}"))?;

    let scope = orbit_engine::resolver::resolve(orbit_core::resolver::ResolveArgs {
        workspace,
        tenant,
        project,
        repository,
    })
    .map_err(|e| e.to_string())?;

    let (_merged, report) = orbit_engine::config::inspect(&scope, eng).map_err(|e| e.to_string())?;

    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("/"));

    let config_file = orbit_engine::launcher::runtime::config_file_path(&scope, eng);
    let context_file = orbit_engine::launcher::runtime::context_file_path(&scope, eng);

    let exec_cmd = match eng {
        Engine::Claude => {
            let ctx = context_file
                .as_ref()
                .map(|p| format!(" --append-system-prompt-file {}", shorten_home(&home, p)))
                .unwrap_or_default();
            let hook_state = orbit_core::engine_hook::EngineHookState::load();
            let catalog = orbit_core::engine_hook::load_all();
            let hooks_suffix =
                if orbit_engine::launcher::engine_hooks::build_settings(&hook_state, &catalog)
                    .is_some()
                {
                    let hooks_path = orbit_engine::launcher::runtime::runtime_dir_for_slug(
                        &scope,
                        Engine::Claude.as_str(),
                    )
                    .join("claude-hooks-settings.json");
                    format!(" --settings {}", shorten_home(&home, &hooks_path))
                } else {
                    String::new()
                };
            format!(
                "claude --mcp-config {}{}{}",
                shorten_home(&home, &config_file),
                ctx,
                hooks_suffix
            )
        }
        Engine::Opencode => {
            format!(
                "OPENCODE_CONFIG={} opencode",
                shorten_home(&home, &config_file)
            )
        }
        Engine::Gemini => {
            format!(
                "GEMINI_CLI_SYSTEM_SETTINGS_PATH={} gemini",
                shorten_home(&home, &config_file)
            )
        }
    };

    let opencode_ws =
        orbit_engine::launcher::runtime::workspace_runtime_dir_for_slug(&scope, "opencode");
    let auth_status = if opencode_ws
        .join("data")
        .join("opencode")
        .join("auth.json")
        .exists()
    {
        "github-copilot ✓".to_string()
    } else if opencode_ws.join("config").join("gh").join("hosts.yml").exists() {
        "github ✓".to_string()
    } else {
        "not configured".to_string()
    };

    let workspace_name = scope
        .workspace_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // ── Plugin context (dynamic injections at launch) ─────────────────────────
    let plugin_state = orbit_core::plugin::PluginState::load();
    let plugins = orbit_core::plugin::load_all();
    let plugin_context: Vec<HarnessPluginContext> = plugins
        .iter()
        .filter(|p| plugin_state.is_enabled(&p.name))
        .filter_map(|p| {
            let ctx = p.context.as_ref()?;
            let prompt_preview = ctx.prompt.as_ref().map(|s| {
                let max = 120;
                if s.len() > max {
                    format!("{}…", &s[..max])
                } else {
                    s.clone()
                }
            });
            Some(HarnessPluginContext {
                name: p.name.clone(),
                prompt_preview,
                instruction_files: ctx.instructions.clone(),
            })
        })
        .collect();

    // ── Activity context preview ──────────────────────────────────────────────
    let activity_scope = orbit_core::activity::scope_key(
        if scope.tenant.is_empty() { None } else { Some(scope.tenant.as_str()) },
        if scope.project.is_empty() { None } else { Some(scope.project.as_str()) },
        if scope.repository.is_empty() { None } else { Some(scope.repository.as_str()) },
    );
    let activity_preview: Vec<String> = if activity_scope.is_empty() {
        vec![]
    } else {
        orbit_core::activity::list(Some(&workspace_name), Some(&activity_scope), None, 5)
            .unwrap_or_default()
            .into_iter()
            .map(|e| format!("[{}] {}", e.ts, e.summary))
            .collect()
    };

    Ok(HarnessReport {
        scope: HarnessScopeInfo {
            workspace: workspace_name,
            workspace_root: shorten_home(&home, &scope.workspace_root),
            tenant: scope.tenant.clone(),
            project: scope.project.clone(),
            repository: scope.repository.clone(),
            engine: eng.as_str().to_string(),
            work_dir: shorten_home(&home, &scope.work_dir),
            exec_cmd,
            auth_status,
        },
        config_layers: report
            .config_layers
            .iter()
            .map(|e| HarnessLayer {
                path: e.path.to_string_lossy().into_owned(),
                exists: e.exists,
                label: e.label.clone(),
            })
            .collect(),
        mcp_layers: report
            .mcp_layers
            .iter()
            .map(|e| HarnessLayer {
                path: e.path.to_string_lossy().into_owned(),
                exists: e.exists,
                label: e.label.clone(),
            })
            .collect(),
        agent_overlay_dirs: report
            .agent_overlay_dirs
            .iter()
            .map(|e| HarnessLayer {
                path: e.path.to_string_lossy().into_owned(),
                exists: e.exists,
                label: e.label.clone(),
            })
            .collect(),
        instructions: report
            .instructions
            .iter()
            .map(|(p, e)| HarnessInstruction {
                path: p.to_string_lossy().into_owned(),
                exists: *e,
            })
            .collect(),
        mcp_servers: report
            .mcp_servers
            .iter()
            .map(|(name, cmd, source)| HarnessMcpServer {
                name: name.clone(),
                command: cmd.clone(),
                source: source.clone(),
            })
            .collect(),
        env_vars: report
            .env_vars
            .iter()
            .map(|(k, v)| {
                let ku = k.to_uppercase();
                let redacted = ku.contains("TOKEN")
                    || ku.contains("SECRET")
                    || ku.contains("PASSWORD")
                    || ku.contains("AUTH");
                HarnessEnvVar {
                    key: k.clone(),
                    value: v.clone(),
                    redacted,
                }
            })
            .collect(),
        commands: report
            .commands
            .iter()
            .map(|(n, s)| HarnessCommand {
                name: n.clone(),
                source: s.clone(),
            })
            .collect(),
        engine_hooks: report
            .engine_hooks
            .iter()
            .map(|(n, e)| HarnessHook {
                name: n.clone(),
                events: e.clone(),
            })
            .collect(),
        plugin_context,
        activity_preview,
    })
}
