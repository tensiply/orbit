use anyhow::Result;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub uptime_secs: Option<u64>,
    pub session_count: usize,
    pub pid: Option<u32>,
}

#[tauri::command]
pub async fn daemon_status() -> Result<DaemonStatus, String> {
    if !orbit_client::ipc::is_available() {
        return Ok(DaemonStatus {
            running: false,
            uptime_secs: None,
            session_count: 0,
            pid: None,
        });
    }
    match orbit_client::ipc::status().await {
        Ok(info) => Ok(DaemonStatus {
            running: true,
            uptime_secs: Some(info.uptime_secs),
            session_count: info.session_count,
            pid: Some(info.pid),
        }),
        Err(_) => Ok(DaemonStatus {
            running: false,
            uptime_secs: None,
            session_count: 0,
            pid: None,
        }),
    }
}

#[tauri::command]
pub async fn daemon_ensure_running(_app: AppHandle) -> Result<(), String> {
    ensure_running(&_app).await.map_err(|e| e.to_string())
}

pub async fn ensure_running(_app: &AppHandle) -> Result<()> {
    if orbit_client::ipc::is_available() {
        return Ok(());
    }
    // Daemon not running — spawn orbitd in background
    let status = tokio::process::Command::new("orbitd")
        .arg("--detach")
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn orbitd: {e}"))?
        .wait()
        .await?;
    if !status.success() {
        tracing::warn!("orbitd exited with status {status}");
    }
    // Give daemon a moment to bind the socket
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    Ok(())
}
