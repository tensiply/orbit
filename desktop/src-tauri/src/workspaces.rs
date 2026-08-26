use orbit_core::workspace_registry::WorkspaceRegistry;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct WorkspaceInfo {
    pub name: String,
    pub slug: String,
    pub is_default: bool,
}

#[tauri::command]
pub fn workspace_list() -> Vec<WorkspaceInfo> {
    WorkspaceRegistry::load()
        .workspaces
        .into_iter()
        .map(|e| WorkspaceInfo {
            name: e.name,
            slug: e.slug,
            is_default: e.is_default,
        })
        .collect()
}
