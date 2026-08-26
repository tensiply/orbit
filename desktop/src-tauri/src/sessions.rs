use anyhow::Result;
use orbit_core::session::Session;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
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
        .filter(|e| e.path().extension().map_or(false, |x| x == "jsonl"))
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
    candidates.sort_by(|a, b| b.0.cmp(&a.0));

    // ai-title is written asynchronously after the first response — scan the full file
    // and keep the last occurrence (Claude Code updates it as conversation continues).
    for (_, path) in candidates {
        if let Ok(f) = std::fs::File::open(&path) {
            let mut last_title: Option<String> = None;
            for line in BufReader::new(f).lines().flatten() {
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
