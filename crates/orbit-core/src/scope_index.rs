use crate::workspace_registry::WorkspaceEntry;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── ScopeIndexEntry ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeIndexEntry {
    pub workspace: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    /// Computed code directory: `$HOME/{workspace}/{tenant}/{project}/{repository}`.
    pub work_dir: PathBuf,
    /// Lowercased tokens derived from all name segments, split on `-` and `_`.
    pub keywords: Vec<String>,
    /// First non-empty line of the repo's source-of-truth README, if present.
    pub description: Option<String>,
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Tokenizes a name segment (split on `-` and `_`, lowercase, filter < 2 chars).
fn tokenize(s: &str) -> Vec<String> {
    s.split(['-', '_'])
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 2)
        .collect()
}

fn extract_keywords(workspace: &str, tenant: &str, project: &str, repository: &str) -> Vec<String> {
    let mut kw = Vec::new();
    for segment in [workspace, tenant, project, repository] {
        kw.extend(tokenize(segment));
        // also include the full lowercased segment as a keyword
        let lower = segment.to_lowercase();
        if !kw.contains(&lower) {
            kw.push(lower);
        }
    }
    kw.sort();
    kw.dedup();
    kw
}

fn read_first_line(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
}

fn try_read_description(ai_root: &Path, tenant: &str, project: &str, repository: &str) -> Option<String> {
    let readme = ai_root
        .join("tenants")
        .join(tenant)
        .join("projects")
        .join(project)
        .join("repositories")
        .join(repository)
        .join("source-of-truth")
        .join("README.md");
    read_first_line(&readme)
}

pub fn scan_dirs(parent: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(parent) else {
        return vec![];
    };
    rd.filter_map(|e| {
        let e = e.ok()?;
        if e.file_type().ok()?.is_dir() {
            Some(e.file_name().to_string_lossy().to_string())
        } else {
            None
        }
    })
    .collect()
}

// ── public API ────────────────────────────────────────────────────────────────

/// Build a flat index of every known repo across all registered workspaces.
/// Scans governance dirs (`ai_root/tenants/*/projects/*/repositories/`).
pub fn build_scope_index(workspaces: &[WorkspaceEntry]) -> Vec<ScopeIndexEntry> {
    let home = home_dir();
    let mut entries = Vec::new();

    for ws in workspaces {
        let tenants_dir = ws.ai_root.join("tenants");
        for tenant in scan_dirs(&tenants_dir) {
            let projects_dir = tenants_dir.join(&tenant).join("projects");
            for project in scan_dirs(&projects_dir) {
                let repos_dir = projects_dir.join(&project).join("repositories");
                for repository in scan_dirs(&repos_dir) {
                    // Code dir: $HOME/{workspace_name}/{tenant}/{project}/{repository}
                    let work_dir = home
                        .join(&ws.name)
                        .join(&tenant)
                        .join(&project)
                        .join(&repository);

                    let keywords = extract_keywords(&ws.name, &tenant, &project, &repository);
                    let description = try_read_description(&ws.ai_root, &tenant, &project, &repository);

                    entries.push(ScopeIndexEntry {
                        workspace: ws.name.clone(),
                        tenant: tenant.clone(),
                        project: project.clone(),
                        repository: repository.clone(),
                        work_dir,
                        keywords,
                        description,
                    });
                }
            }
        }
    }

    entries
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_workspace(dir: &TempDir, name: &str, tenant: &str, project: &str, repo: &str) -> WorkspaceEntry {
        let ai_root = dir.path().join(name).join("AI");
        let repo_dir = ai_root
            .join("tenants").join(tenant)
            .join("projects").join(project)
            .join("repositories").join(repo)
            .join("source-of-truth");
        fs::create_dir_all(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), format!("# {repo}\nDoes CDC query stuff.")).unwrap();
        WorkspaceEntry {
            name: name.to_string(),
            slug: name.to_lowercase(),
            ai_root,
            is_default: false,
        }
    }

    #[test]
    fn indexes_repos_from_governance() {
        let dir = TempDir::new().unwrap();
        let ws = make_workspace(&dir, "BeFra", "JAFRAMX", "INTERFACES", "jf-cdc-interfaces");
        let index = build_scope_index(&[ws]);
        assert_eq!(index.len(), 1);
        let entry = &index[0];
        assert_eq!(entry.workspace, "BeFra");
        assert_eq!(entry.tenant, "JAFRAMX");
        assert_eq!(entry.repository, "jf-cdc-interfaces");
    }

    #[test]
    fn keywords_include_name_parts() {
        let dir = TempDir::new().unwrap();
        let ws = make_workspace(&dir, "BeFra", "JAFRAMX", "INTERFACES", "jf-cdc-interfaces");
        let index = build_scope_index(&[ws]);
        let kw = &index[0].keywords;
        assert!(kw.contains(&"jf".to_string()));
        assert!(kw.contains(&"cdc".to_string()));
        assert!(kw.contains(&"interfaces".to_string()));
    }

    #[test]
    fn reads_description_from_readme() {
        let dir = TempDir::new().unwrap();
        let ws = make_workspace(&dir, "BeFra", "JAFRAMX", "INTERFACES", "jf-cdc-interfaces");
        let index = build_scope_index(&[ws]);
        assert_eq!(index[0].description.as_deref(), Some("Does CDC query stuff."));
    }

    #[test]
    fn empty_when_no_tenants() {
        let dir = TempDir::new().unwrap();
        let ai_root = dir.path().join("AI");
        fs::create_dir_all(ai_root.join("tenants")).unwrap();
        let ws = WorkspaceEntry {
            name: "AI".to_string(),
            slug: "ai".to_string(),
            ai_root,
            is_default: true,
        };
        let index = build_scope_index(&[ws]);
        assert!(index.is_empty());
    }
}
