use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub installed: bool,
    pub mcp_enabled: bool,
}

/// List plugins by shelling out to `orbit plugins list --json`.
/// Phase 3 will add direct orbit-core integration.
#[tauri::command]
pub async fn plugin_list() -> Result<Vec<PluginInfo>, String> {
    tokio::task::spawn_blocking(|| {
        let output = Command::new("orbit")
            .args(["plugins", "list", "--json"])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let plugins: Vec<PluginInfo> =
            serde_json::from_slice(&output.stdout).unwrap_or_default();
        Ok(plugins)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn plugin_enable(name: String, scope: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let output = Command::new("orbit")
            .args(["plugins", "enable", &name, "--scope", &scope])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn plugin_disable(name: String, scope: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let output = Command::new("orbit")
            .args(["plugins", "disable", &name, "--scope", &scope])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
