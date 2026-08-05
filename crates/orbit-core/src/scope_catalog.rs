use crate::{
    data_paths::scope_catalog_path,
    scope_index::{build_scope_index, scan_dirs, ScopeIndexEntry},
    workspace_registry::WorkspaceEntry,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

// ── catalog ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCatalog {
    pub scanned_at: u64,
    pub workspace_count: usize,
    pub entries: Vec<ScopeIndexEntry>,
}

impl ScopeCatalog {
    pub fn scan(workspaces: &[WorkspaceEntry]) -> Self {
        let entries = build_scope_index(workspaces);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            scanned_at: now,
            workspace_count: workspaces.len(),
            entries,
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = scope_catalog_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn load() -> Option<Self> {
        let text = fs::read_to_string(scope_catalog_path()).ok()?;
        serde_json::from_str(&text).ok()
    }
}

// ── governance health check ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeIssue {
    /// Human-readable scope path, e.g. "BeFra/JAFRAMX/INTERFACES/jf-cdc-interfaces"
    pub scope: String,
    pub issue: String,
}

/// Validate governance file completeness across all levels of all workspaces.
pub fn check_workspaces(workspaces: &[WorkspaceEntry]) -> Vec<ScopeIssue> {
    let mut issues = Vec::new();

    for ws in workspaces {
        let ai_root = &ws.ai_root;

        // Workspace level: orbit.json in ai_root
        check_file(ai_root, "orbit.json", &ws.name, &mut issues);

        let tenants_dir = ai_root.join("tenants");
        if !tenants_dir.is_dir() {
            continue;
        }

        for tenant in valid_scope_names(&tenants_dir) {
            let tenant_dir = tenants_dir.join(&tenant);
            let t_scope = format!("{}/{}", ws.name, tenant);

            check_file(&tenant_dir, "orbit.json", &t_scope, &mut issues);
            check_sot(&tenant_dir, "README.md", &t_scope, &mut issues);

            let projects_dir = tenant_dir.join("projects");
            if !projects_dir.is_dir() {
                continue;
            }

            for project in valid_scope_names(&projects_dir) {
                let project_dir = projects_dir.join(&project);
                let p_scope = format!("{t_scope}/{project}");

                check_file(&project_dir, "orbit.json", &p_scope, &mut issues);
                check_sot(&project_dir, "README.md", &p_scope, &mut issues);

                let repos_dir = project_dir.join("repositories");
                if !repos_dir.is_dir() {
                    continue;
                }

                for repo in valid_scope_names(&repos_dir) {
                    let repo_dir = repos_dir.join(&repo);
                    let r_scope = format!("{p_scope}/{repo}");

                    check_file(&repo_dir, "orbit.json", &r_scope, &mut issues);
                    check_sot(&repo_dir, "README.md", &r_scope, &mut issues);
                    check_sot(&repo_dir, "conventions.md", &r_scope, &mut issues);
                }
            }
        }
    }

    issues
}

/// Returns subdirectory names that are valid orbit scope identifiers:
/// no leading dot, no whitespace, non-empty.
fn valid_scope_names(parent: &Path) -> Vec<String> {
    scan_dirs(parent)
        .into_iter()
        .filter(|name| {
            !name.is_empty()
                && !name.starts_with('.')
                && !name.contains(char::is_whitespace)
        })
        .collect()
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn check_file(dir: &Path, filename: &str, scope: &str, issues: &mut Vec<ScopeIssue>) {
    if !dir.join(filename).is_file() {
        issues.push(ScopeIssue {
            scope: scope.to_string(),
            issue: format!("missing {filename}"),
        });
    }
}

fn check_sot(dir: &Path, filename: &str, scope: &str, issues: &mut Vec<ScopeIssue>) {
    if !dir.join("source-of-truth").join(filename).is_file() {
        issues.push(ScopeIssue {
            scope: scope.to_string(),
            issue: format!("missing source-of-truth/{filename}"),
        });
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_complete_repo(dir: &TempDir, ws: &str, tenant: &str, project: &str, repo: &str) -> WorkspaceEntry {
        let ai_root = dir.path().join("AI");

        // workspace orbit.json
        fs::create_dir_all(&ai_root).unwrap();
        fs::write(ai_root.join("orbit.json"), "{}").unwrap();

        let tenant_dir = ai_root.join("tenants").join(tenant);
        let sot = tenant_dir.join("source-of-truth");
        fs::create_dir_all(&sot).unwrap();
        fs::write(tenant_dir.join("orbit.json"), "{}").unwrap();
        fs::write(sot.join("README.md"), "# Tenant").unwrap();

        let project_dir = tenant_dir.join("projects").join(project);
        let sot = project_dir.join("source-of-truth");
        fs::create_dir_all(&sot).unwrap();
        fs::write(project_dir.join("orbit.json"), "{}").unwrap();
        fs::write(sot.join("README.md"), "# Project").unwrap();

        let repo_dir = project_dir.join("repositories").join(repo);
        let sot = repo_dir.join("source-of-truth");
        fs::create_dir_all(&sot).unwrap();
        fs::write(repo_dir.join("orbit.json"), "{}").unwrap();
        fs::write(sot.join("README.md"), "# Repo").unwrap();
        fs::write(sot.join("conventions.md"), "# Conventions").unwrap();

        WorkspaceEntry {
            name: ws.to_string(),
            slug: ws.to_lowercase(),
            ai_root,
            is_default: true,
        }
    }

    #[test]
    fn complete_scope_has_no_issues() {
        let dir = TempDir::new().unwrap();
        let ws = make_complete_repo(&dir, "AI", "AIDEV", "AI-ECOSYSTEM", "orbit");
        let issues = check_workspaces(&[ws]);
        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn missing_orbit_json_at_repo_level_is_reported() {
        let dir = TempDir::new().unwrap();
        let ws = make_complete_repo(&dir, "AI", "AIDEV", "AI-ECOSYSTEM", "orbit");
        // Remove repo-level orbit.json
        fs::remove_file(
            ws.ai_root
                .join("tenants/AIDEV/projects/AI-ECOSYSTEM/repositories/orbit/orbit.json"),
        )
        .unwrap();
        let issues = check_workspaces(&[ws]);
        assert!(issues.iter().any(|i| i.issue.contains("orbit.json")));
    }

    #[test]
    fn catalog_round_trip() {
        let dir = TempDir::new().unwrap();
        let ws = make_complete_repo(&dir, "AI", "AIDEV", "AI-ECOSYSTEM", "orbit");
        let catalog = ScopeCatalog::scan(&[ws]);
        assert_eq!(catalog.entries.len(), 1);
        assert_eq!(catalog.workspace_count, 1);
    }
}
