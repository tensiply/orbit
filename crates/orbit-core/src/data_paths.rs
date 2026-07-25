use std::path::PathBuf;

// ── helpers ───────────────────────────────────────────────────────────────────

pub fn orbit_data_root() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("orbit")
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share/orbit"))
            .unwrap_or_else(|| PathBuf::from("/tmp/orbit"))
    }
}

/// Derive a filesystem-safe slug from a workspace name.
/// "AI" → "ai", "BeFra" → "befra", "My Workspace" → "my-workspace".
pub fn slugify(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    s.trim_matches('-').to_string()
}

// ── per-workspace paths ───────────────────────────────────────────────────────

/// Plans directory for a workspace name.
/// `None` → legacy flat: `<data_root>/plans/`
/// `Some("AI")` → scoped: `<data_root>/workspaces/ai/plans/`
pub fn plans_dir_for(workspace_name: Option<&str>) -> PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("plans"),
        Some(name) => root.join("workspaces").join(slugify(name)).join("plans"),
    }
}

pub fn memory_path_for(workspace_name: Option<&str>) -> PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("memory/plan_runs.jsonl"),
        Some(name) => root
            .join("workspaces")
            .join(slugify(name))
            .join("memory/plan_runs.jsonl"),
    }
}

pub fn audit_path_for(workspace_name: Option<&str>) -> PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("audit.jsonl"),
        Some(name) => root
            .join("workspaces")
            .join(slugify(name))
            .join("audit.jsonl"),
    }
}

pub fn schedules_path_for(workspace_name: Option<&str>) -> PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("schedules.json"),
        Some(name) => root
            .join("workspaces")
            .join(slugify(name))
            .join("schedules.json"),
    }
}

pub fn activity_index_path_for(workspace_name: Option<&str>) -> std::path::PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("activity/index.jsonl"),
        Some(name) => root
            .join("workspaces")
            .join(slugify(name))
            .join("activity/index.jsonl"),
    }
}

pub fn tasks_index_path_for(workspace_name: Option<&str>) -> PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("tasks/index.jsonl"),
        Some(name) => root
            .join("workspaces")
            .join(slugify(name))
            .join("tasks/index.jsonl"),
    }
}

// ── document paths ────────────────────────────────────────────────────────────

/// User-defined document templates: `~/.local/share/orbit/templates/documents/`
pub fn document_templates_dir() -> PathBuf {
    orbit_data_root().join("templates/documents")
}

/// Workspace-scoped document templates: `$AI_CONTEXT_ROOT/templates/document/`
///
/// Returns `None` when `AI_CONTEXT_ROOT` is not set (outside an orbit session).
pub fn workspace_document_templates_dir() -> Option<PathBuf> {
    std::env::var("AI_CONTEXT_ROOT")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|root| PathBuf::from(root).join("templates/document"))
}

/// User document rule overrides: `~/.config/orbit/document-rules/`
pub fn document_rules_dir() -> PathBuf {
    if let Ok(orbit_home) = std::env::var("ORBIT_CONFIG_HOME") {
        return PathBuf::from(orbit_home).join("document-rules");
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("orbit/document-rules")
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".config/orbit/document-rules"))
            .unwrap_or_else(|| PathBuf::from("/tmp/orbit/document-rules"))
    }
}

/// Root for user-visible generated documents: `~/.orbit/documents/`
/// (NOT XDG data home — this is an intentionally accessible location).
pub fn documents_output_base() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".orbit/documents"))
        .unwrap_or_else(|| PathBuf::from("/tmp/orbit/documents"))
}

/// Scope-based output directory: `~/.orbit/documents/{workspace}/{tenant}/{project}/{repo}/`
pub fn documents_scope_dir(workspace: &str, tenant: &str, project: &str, repo: &str) -> PathBuf {
    let mut path = documents_output_base();
    for part in [workspace, tenant, project, repo] {
        if !part.is_empty() {
            path = path.join(part);
        }
    }
    path
}

/// NDJSON index of document entries for a workspace.
pub fn documents_index_path_for(workspace_name: Option<&str>) -> PathBuf {
    let root = orbit_data_root();
    match workspace_name.filter(|s| !s.is_empty()) {
        None => root.join("documents/index.jsonl"),
        Some(name) => root
            .join("workspaces")
            .join(slugify(name))
            .join("documents/index.jsonl"),
    }
}

// ── cross-workspace discovery ─────────────────────────────────────────────────

/// All plans directories that exist on disk: legacy flat + every `workspaces/*/plans/`.
pub fn all_plans_dirs() -> Vec<PathBuf> {
    let root = orbit_data_root();
    let mut dirs = vec![root.join("plans")];

    let ws_root = root.join("workspaces");
    if let Ok(entries) = std::fs::read_dir(&ws_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let plans = entry.path().join("plans");
                dirs.push(plans);
            }
        }
    }
    dirs
}

/// All memory JSONL paths that exist on disk: legacy flat + every `workspaces/*/memory/plan_runs.jsonl`.
pub fn all_memory_paths() -> Vec<PathBuf> {
    let root = orbit_data_root();
    let mut paths = vec![root.join("memory/plan_runs.jsonl")];

    let ws_root = root.join("workspaces");
    if let Ok(entries) = std::fs::read_dir(&ws_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                paths.push(entry.path().join("memory/plan_runs.jsonl"));
            }
        }
    }
    paths
}

/// All tasks JSONL paths: legacy flat + every `workspaces/*/tasks/index.jsonl`.
pub fn all_tasks_paths() -> Vec<PathBuf> {
    let root = orbit_data_root();
    let mut paths = vec![root.join("tasks/index.jsonl")];

    let ws_root = root.join("workspaces");
    if let Ok(entries) = std::fs::read_dir(&ws_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                paths.push(entry.path().join("tasks/index.jsonl"));
            }
        }
    }
    paths
}

/// All audit JSONL paths that exist on disk: legacy flat + every `workspaces/*/audit.jsonl`.
pub fn all_audit_paths() -> Vec<PathBuf> {
    let root = orbit_data_root();
    let mut paths = vec![root.join("audit.jsonl")];

    let ws_root = root.join("workspaces");
    if let Ok(entries) = std::fs::read_dir(&ws_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                paths.push(entry.path().join("audit.jsonl"));
            }
        }
    }
    paths
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("AI"), "ai");
        assert_eq!(slugify("BeFra"), "befra");
        assert_eq!(slugify("My Workspace"), "my-workspace");
        assert_eq!(slugify("--test--"), "test");
    }

    #[test]
    fn plans_dir_none_is_legacy() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "/tmp/orbit-test-data");
        }
        assert_eq!(
            plans_dir_for(None),
            PathBuf::from("/tmp/orbit-test-data/orbit/plans")
        );
    }

    #[test]
    fn plans_dir_named_workspace() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "/tmp/orbit-test-data");
        }
        assert_eq!(
            plans_dir_for(Some("AI")),
            PathBuf::from("/tmp/orbit-test-data/orbit/workspaces/ai/plans")
        );
        assert_eq!(
            plans_dir_for(Some("BeFra")),
            PathBuf::from("/tmp/orbit-test-data/orbit/workspaces/befra/plans")
        );
    }

    #[test]
    fn empty_workspace_name_is_legacy() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "/tmp/orbit-test-data");
        }
        assert_eq!(plans_dir_for(Some("")), plans_dir_for(None));
    }
}
