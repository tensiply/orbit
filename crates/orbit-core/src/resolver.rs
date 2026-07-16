use crate::{context::OrbitScope, user_config::UserConfig};
use anyhow::{Result, bail};
use directories::BaseDirs;
use std::path::{Path, PathBuf};

/// Arguments for scope resolution — direct mapping from CLI flags.
#[derive(Debug, Default)]
pub struct ResolveArgs {
    pub workspace: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
}

/// Public entry point. Resolves args against the real filesystem.
/// Reads `ai_root` from `~/.config/orbit/config.toml` (falls back to `~/AI`).
pub fn resolve(args: ResolveArgs) -> Result<OrbitScope> {
    let base_dirs =
        BaseDirs::new().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let ai_root = UserConfig::load().ai_root_expanded();
    resolve_inner(args, base_dirs.home_dir(), &ai_root)
}

/// Testable core: accepts explicit home and ai_root paths.
pub fn resolve_with_home(args: ResolveArgs, home: &Path) -> Result<OrbitScope> {
    let ai_root = home.join("AI");
    resolve_inner(args, home, &ai_root)
}

/// Testable core with fully explicit paths.
pub fn resolve_with_roots(args: ResolveArgs, home: &Path, ai_root: &Path) -> Result<OrbitScope> {
    resolve_inner(args, home, ai_root)
}

/// Resolve scope from the current working directory.
///
/// Walks the cwd's ancestors to find a workspace root (a direct child of home
/// that contains `tenants/` or `orbit.toml`), then maps remaining path
/// segments to tenant / project / repository.
pub fn resolve_from_cwd() -> Result<OrbitScope> {
    let base_dirs =
        BaseDirs::new().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let home = base_dirs.home_dir();
    let cwd = std::env::current_dir().unwrap_or_else(|_| home.to_path_buf());
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();
    resolve_from_path_inner(&cwd, home, &ai_root)
}

/// Like `resolve_from_cwd` but uses the git repository root as anchor when
/// available. This handles subdirectory invocations correctly — running
/// `orbit plan` from `src/` inside a repo resolves to the repo root scope,
/// not a deeper scope.
pub fn resolve_from_git_or_cwd() -> Result<OrbitScope> {
    let base_dirs =
        BaseDirs::new().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let home = base_dirs.home_dir();
    let anchor = git_repo_root()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| home.to_path_buf()));
    let ai_root = UserConfig::load().ai_root_expanded();
    resolve_from_path_inner(&anchor, home, &ai_root)
}

/// Testable variant of cwd resolution.
pub fn resolve_from_path(cwd: &Path, home: &Path, ai_root: &Path) -> Result<OrbitScope> {
    resolve_from_path_inner(cwd, home, ai_root)
}

/// Convert an `OrbitScope` to the `(workspace, tenant, project, repository)` tuple
/// used by plan commands. Empty strings become `None`.
pub fn scope_to_tuple(
    scope: &OrbitScope,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let workspace = scope
        .workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let tenant = if scope.tenant.is_empty() {
        None
    } else {
        Some(scope.tenant.clone())
    };
    let project = if scope.project.is_empty() {
        None
    } else {
        Some(scope.project.clone())
    };
    let repository = if scope.repository.is_empty() {
        None
    } else {
        Some(scope.repository.clone())
    };
    (workspace, tenant, project, repository)
}

fn resolve_from_path_inner(cwd: &Path, home: &Path, ai_root: &Path) -> Result<OrbitScope> {
    // Find the direct child of home that contains cwd.
    let workspace_root = {
        let Ok(rd) = std::fs::read_dir(home) else {
            anyhow::bail!("cannot read home directory");
        };
        let mut candidates: Vec<std::path::PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir() && cwd.starts_with(p))
            .collect();
        // Prefer the deepest match (shouldn't be more than one at home level)
        candidates.sort_by_key(|p| p.components().count());
        candidates.into_iter().last().ok_or_else(|| {
            anyhow::anyhow!(
                "current directory is not inside any workspace under {}",
                home.display()
            )
        })?
    };

    let ai_name = ai_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AI".to_string());

    // Strip workspace root prefix, then skip the AI layer dir if present.
    let rest = cwd.strip_prefix(&workspace_root).unwrap_or(Path::new(""));
    let mut segments = rest
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => n.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .peekable();

    // Skip the AI directory itself if it is the first segment.
    if segments
        .peek()
        .map(|s| s.eq_ignore_ascii_case(&ai_name))
        .unwrap_or(false)
    {
        segments.next();
    }

    let ai_context_root = {
        let candidate = workspace_root.join(&ai_name);
        if candidate.join("tenants").is_dir() {
            candidate
        } else {
            workspace_root.clone()
        }
    };

    let tenants_root = ai_context_root.join("tenants");

    // Skip "tenants" directory if it appears next in the path.
    if segments
        .peek()
        .map(|s| s.eq_ignore_ascii_case("tenants"))
        .unwrap_or(false)
    {
        segments.next();
    }

    let tenant_raw = segments.next().unwrap_or_default();
    let project_raw = segments.next().unwrap_or_default();
    let repo_raw = segments.next().unwrap_or_default();

    let tenant = if tenant_raw.is_empty() {
        String::new()
    } else {
        resolve_name(&tenants_root, &tenant_raw)
    };

    let project = if project_raw.is_empty() || tenant.is_empty() {
        String::new()
    } else {
        let projects_root = tenants_root.join(&tenant).join("projects");
        // Skip "projects" segment if cwd went through it
        let project_input = if project_raw.eq_ignore_ascii_case("projects") {
            segments.next().unwrap_or_default()
        } else {
            project_raw
        };
        if project_input.is_empty() {
            String::new()
        } else {
            resolve_name(&projects_root, &project_input)
        }
    };

    let repository = if repo_raw.is_empty() || project.is_empty() {
        String::new()
    } else {
        let repos_root = tenants_root
            .join(&tenant)
            .join("projects")
            .join(&project)
            .join("repositories");
        let repo_input = if repo_raw.eq_ignore_ascii_case("repositories") {
            segments.next().unwrap_or_default()
        } else {
            repo_raw
        };
        if repo_input.is_empty() {
            String::new()
        } else {
            resolve_name(&repos_root, &repo_input)
        }
    };

    let code_root = workspace_root.join(&tenant);
    let tenant_dir = ai_context_root.join("tenants").join(&tenant);

    let work_dir = if !repository.is_empty() {
        code_root.join(&project).join(&repository)
    } else if !project.is_empty() {
        code_root.join(&project)
    } else if !tenant.is_empty() {
        code_root.clone()
    } else {
        workspace_root.clone()
    };

    Ok(OrbitScope {
        workspace_root,
        ai_context_root,
        global_ai_root: ai_root.to_path_buf(),
        tenant,
        project,
        repository,
        tenant_dir,
        code_root,
        work_dir,
        global_mode: false,
    })
}

fn resolve_inner(args: ResolveArgs, home: &Path, ai_root: &Path) -> Result<OrbitScope> {
    // ── global mode: no arguments at all ─────────────────────────────────────
    if args.workspace.is_none() {
        if !ai_root.is_dir() {
            bail!(
                "AI root not found: {}\nRun `orbit setup` to configure or `orbit init` to clone the governance repo.",
                ai_root.display()
            );
        }
        return Ok(OrbitScope {
            ai_context_root: ai_root.to_path_buf(),
            global_ai_root: ai_root.to_path_buf(),
            code_root: ai_root.to_path_buf(),
            work_dir: ai_root.to_path_buf(),
            workspace_root: ai_root.to_path_buf(),
            global_mode: true,
            ..Default::default()
        });
    }

    // ── resolve workspace root ────────────────────────────────────────────────
    let workspace_str = args.workspace.as_deref().unwrap();
    let workspace_root = find_dir_icase(home, workspace_str)
        .ok_or_else(|| anyhow::anyhow!("workspace not found: {workspace_str}"))?;

    // AI_CONTEXT_ROOT: prefer WORKSPACE_ROOT/<ai_root_name> when it has tenants/
    let ai_root_name = ai_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AI".to_string());

    let ai_context_root = {
        let candidate = workspace_root.join(&ai_root_name);
        if candidate.join("tenants").is_dir() {
            candidate
        } else {
            workspace_root.clone()
        }
    };

    let tenants_root = ai_context_root.join("tenants");

    // ── resolve tenant ────────────────────────────────────────────────────────
    let user_cfg = UserConfig::load();
    let default_tenant = if user_cfg.engine.default_tenant.is_empty() {
        "AI"
    } else {
        &user_cfg.engine.default_tenant
    };
    let tenant_input = args.tenant.as_deref().unwrap_or(default_tenant);
    let tenant = resolve_name(&tenants_root, tenant_input);

    // Resolve code_root with icase so tenant names in SOT and on-disk don't need to match.
    let code_root =
        find_dir_icase(&workspace_root, &tenant).unwrap_or_else(|| workspace_root.join(&tenant));

    // ── resolve project ───────────────────────────────────────────────────────
    let project = match &args.project {
        None => String::new(),
        Some(p) => {
            let projects_root = tenants_root.join(&tenant).join("projects");
            resolve_name_with_code_fallback(&projects_root, &code_root, p)
        }
    };

    // ── resolve repository ────────────────────────────────────────────────────
    let repository = match (&args.repository, project.is_empty()) {
        (Some(r), false) => {
            let repos_root = tenants_root
                .join(&tenant)
                .join("projects")
                .join(&project)
                .join("repositories");
            let repo_code_root = code_root.join(&project);
            resolve_name_with_code_fallback(&repos_root, &repo_code_root, r)
        }
        _ => String::new(),
    };

    // ── derive paths ──────────────────────────────────────────────────────────
    let tenant_dir = ai_context_root.join("tenants").join(&tenant);

    let work_dir = if !repository.is_empty() {
        code_root.join(&project).join(&repository)
    } else if !project.is_empty() {
        code_root.join(&project)
    } else {
        code_root.clone()
    };
    Ok(OrbitScope {
        workspace_root,
        ai_context_root,
        global_ai_root: ai_root.to_path_buf(),
        tenant,
        project,
        repository,
        tenant_dir,
        code_root,
        work_dir,
        global_mode: false,
    })
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Like `resolve_name` but falls back to a case-insensitive lookup in `code_root`
/// when `sot_root` has no matching entry. Auto-creates the SOT directory when the
/// name is found only in code, so subsequent launches find it in SOT.
fn resolve_name_with_code_fallback(sot_root: &Path, code_root: &Path, target: &str) -> String {
    if let Some(name) = find_dir_icase(sot_root, target)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
    {
        return name;
    }
    if let Some(name) = find_dir_icase(code_root, target)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
    {
        let _ = std::fs::create_dir_all(sot_root.join(&name));
        return name;
    }
    target.to_string()
}

/// Find a direct subdirectory of `root` whose name matches `target` ignoring case.
/// Returns the original on-disk name, or `target` itself if not found.
fn resolve_name(root: &Path, target: &str) -> String {
    find_dir_icase(root, target)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| target.to_string())
}

/// Find a direct subdirectory of `root` whose lowercased name equals
/// `target.to_lowercase()`. Returns the full path on success.
pub fn find_dir_icase(root: &Path, target: &str) -> Option<PathBuf> {
    let needle = target.to_lowercase();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase() == needle)
                .unwrap_or(false)
        })
        .collect();

    entries.sort();
    entries.into_iter().next()
}

/// Returns the git repository root for the current directory, if inside a git repo.
fn git_repo_root() -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?;
        Some(PathBuf::from(path.trim()))
    } else {
        None
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn fake_home() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let repo = tmp
            .path()
            .join("AI/tenants/AIDEV/projects/AI-ECOSYSTEM/repositories/orbit");
        fs::create_dir_all(&repo).unwrap();
        tmp
    }

    #[test]
    fn global_mode_opens_ai_root() {
        let home = fake_home();
        let scope = resolve_with_home(ResolveArgs::default(), home.path()).unwrap();
        assert!(scope.global_mode);
        assert_eq!(scope.workspace_root, home.path().join("AI"));
        assert!(scope.tenant.is_empty());
    }

    #[test]
    fn resolves_workspace_case_insensitive() {
        let home = fake_home();
        let args = ResolveArgs {
            workspace: Some("ai".to_string()),
            ..Default::default()
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        assert_eq!(scope.workspace_root, home.path().join("AI"));
    }

    #[test]
    fn resolves_tenant_case_insensitive() {
        let home = fake_home();
        let args = ResolveArgs {
            workspace: Some("ai".to_string()),
            tenant: Some("aidev".to_string()),
            ..Default::default()
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        assert_eq!(scope.tenant, "AIDEV");
    }

    #[test]
    fn resolves_full_scope() {
        let home = fake_home();
        let args = ResolveArgs {
            workspace: Some("AI".to_string()),
            tenant: Some("AIDEV".to_string()),
            project: Some("ai-ecosystem".to_string()),
            repository: Some("ORBIT".to_string()),
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        assert_eq!(scope.tenant, "AIDEV");
        assert_eq!(scope.project, "AI-ECOSYSTEM");
        assert_eq!(scope.repository, "orbit");
        assert_eq!(
            scope.work_dir,
            home.path().join("AI/AIDEV/AI-ECOSYSTEM/orbit")
        );
    }

    #[test]
    fn missing_workspace_returns_error() {
        let home = fake_home();
        let args = ResolveArgs {
            workspace: Some("nonexistent".to_string()),
            ..Default::default()
        };
        assert!(resolve_with_home(args, home.path()).is_err());
    }

    #[test]
    fn work_dir_falls_back_to_project_when_no_repo() {
        let home = fake_home();
        let args = ResolveArgs {
            workspace: Some("AI".to_string()),
            tenant: Some("AIDEV".to_string()),
            project: Some("AI-ECOSYSTEM".to_string()),
            repository: None,
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        assert_eq!(scope.work_dir, home.path().join("AI/AIDEV/AI-ECOSYSTEM"));
    }

    #[test]
    fn resolves_project_from_code_when_not_in_sot() {
        let home = TempDir::new().unwrap();
        let code_project = home.path().join("BeFra/JAFRAUS/MULESOFT");
        fs::create_dir_all(&code_project).unwrap();
        fs::create_dir_all(home.path().join("BeFra/AI/tenants/JAFRAUS/projects")).unwrap();

        let args = ResolveArgs {
            workspace: Some("befra".to_string()),
            tenant: Some("jafraus".to_string()),
            project: Some("mulesoft".to_string()),
            repository: None,
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        assert_eq!(scope.project, "MULESOFT");
        assert_eq!(scope.work_dir, code_project);
        assert!(
            home.path()
                .join("BeFra/AI/tenants/JAFRAUS/projects/MULESOFT")
                .is_dir()
        );
    }

    #[test]
    fn resolves_repository_from_code_when_not_in_sot() {
        let home = TempDir::new().unwrap();
        let code_repo = home
            .path()
            .join("BeFra/JAFRAUS/MULESOFT/jafra-us-mulesoft-file-transfer");
        fs::create_dir_all(&code_repo).unwrap();
        fs::create_dir_all(
            home.path()
                .join("BeFra/AI/tenants/JAFRAUS/projects/MULESOFT/repositories"),
        )
        .unwrap();

        let args = ResolveArgs {
            workspace: Some("befra".to_string()),
            tenant: Some("jafraus".to_string()),
            project: Some("mulesoft".to_string()),
            repository: Some("jafra-us-mulesoft-file-transfer".to_string()),
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        assert_eq!(scope.project, "MULESOFT");
        assert_eq!(scope.repository, "jafra-us-mulesoft-file-transfer");
        assert_eq!(scope.work_dir, code_repo);
        assert!(home
            .path()
            .join("BeFra/AI/tenants/JAFRAUS/projects/MULESOFT/repositories/jafra-us-mulesoft-file-transfer")
            .is_dir());
    }

    #[test]
    fn custom_ai_root_is_used() {
        let home = TempDir::new().unwrap();
        let ai_root = home.path().join("MyAI");
        fs::create_dir_all(ai_root.join("tenants/CORP/projects/P/repositories/R")).unwrap();

        let args = ResolveArgs::default();
        let scope = resolve_with_roots(args, home.path(), &ai_root).unwrap();
        assert!(scope.global_mode);
        assert_eq!(scope.global_ai_root, ai_root);
    }

    #[test]
    fn scope_to_tuple_extracts_fields() {
        let home = fake_home();
        let args = ResolveArgs {
            workspace: Some("AI".to_string()),
            tenant: Some("AIDEV".to_string()),
            project: Some("AI-ECOSYSTEM".to_string()),
            repository: Some("orbit".to_string()),
        };
        let scope = resolve_with_home(args, home.path()).unwrap();
        let (ws, t, p, r) = scope_to_tuple(&scope);
        assert_eq!(ws.as_deref(), Some("AI"));
        assert_eq!(t.as_deref(), Some("AIDEV"));
        assert_eq!(p.as_deref(), Some("AI-ECOSYSTEM"));
        assert_eq!(r.as_deref(), Some("orbit"));
    }

    #[test]
    fn resolve_from_path_finds_scope() {
        let home = fake_home();
        let cwd = home.path().join("AI/AIDEV/AI-ECOSYSTEM/orbit");
        let ai_root = home.path().join("AI");
        let scope = resolve_from_path(&cwd, home.path(), &ai_root).unwrap();
        assert_eq!(scope.tenant, "AIDEV");
        assert_eq!(scope.project, "AI-ECOSYSTEM");
        assert_eq!(scope.repository, "orbit");
    }

    #[test]
    fn resolve_from_path_workspace_root_as_cwd() {
        // cwd == workspace root — should resolve with empty tenant/project/repo
        let home = TempDir::new().unwrap();
        let ai_root = home.path().join("AI");
        fs::create_dir_all(ai_root.join("tenants")).unwrap();
        let befra = home.path().join("BeFra");
        fs::create_dir_all(&befra).unwrap();

        let scope = resolve_from_path(&befra, home.path(), &ai_root).unwrap();
        assert_eq!(scope.workspace_root, befra);
        assert_eq!(scope.tenant, "");
        assert_eq!(scope.project, "");
        assert_eq!(scope.repository, "");
    }
}
