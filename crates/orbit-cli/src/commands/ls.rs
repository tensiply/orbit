use anyhow::Result;
use clap::Args;
use directories::BaseDirs;
use orbit_core::user_config::UserConfig;
use std::{fs, path::Path};

#[derive(Debug, Args)]
pub struct LsArgs {
    /// Workspace name — lists tenants. Omit to list workspaces.
    pub workspace: Option<String>,
    /// Tenant name — lists projects. Omit to list tenants.
    pub tenant: Option<String>,
    /// Project name — lists repositories. Omit to list projects.
    pub project: Option<String>,
}

pub fn run(args: LsArgs) -> Result<()> {
    let base = BaseDirs::new().expect("cannot determine home directory");
    let home = base.home_dir();
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();
    let ai_name = ai_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AI".to_string());

    match (&args.workspace, &args.tenant, &args.project) {
        (None, _, _) => {
            let workspaces = workspace_names(home);
            if workspaces.is_empty() {
                println!("No workspaces found in {}", home.display());
            } else {
                println!("Workspaces in {}:", home.display());
                for w in &workspaces {
                    println!("  {w}");
                }
            }
        }
        (Some(ws), None, _) => {
            let ws_root = find_icase(home, ws).unwrap_or_else(|| home.join(ws));
            let tenants_dir = tenants_root(&ws_root, &ai_name);
            list_dir(&tenants_dir, &format!("Tenants in {ws}:"));
        }
        (Some(ws), Some(tenant), None) => {
            let ws_root = find_icase(home, ws).unwrap_or_else(|| home.join(ws));
            let tenants_dir = tenants_root(&ws_root, &ai_name);
            let tenant_dir =
                find_icase(&tenants_dir, tenant).unwrap_or_else(|| tenants_dir.join(tenant));
            list_dir(
                &tenant_dir.join("projects"),
                &format!("Projects in {ws}/{tenant}:"),
            );
        }
        (Some(ws), Some(tenant), Some(project)) => {
            let ws_root = find_icase(home, ws).unwrap_or_else(|| home.join(ws));
            let tenants_dir = tenants_root(&ws_root, &ai_name);
            let tenant_dir =
                find_icase(&tenants_dir, tenant).unwrap_or_else(|| tenants_dir.join(tenant));
            let project_dir = find_icase(&tenant_dir.join("projects"), project)
                .unwrap_or_else(|| tenant_dir.join("projects").join(project));
            list_dir(
                &project_dir.join("repositories"),
                &format!("Repos in {ws}/{tenant}/{project}:"),
            );
        }
    }

    Ok(())
}

fn tenants_root(ws_root: &Path, ai_name: &str) -> std::path::PathBuf {
    let candidate = ws_root.join(ai_name).join("tenants");
    if candidate.is_dir() {
        candidate
    } else {
        ws_root.join("tenants")
    }
}

fn list_dir(dir: &Path, header: &str) {
    let entries = subdirs(dir);
    println!("{header}");
    if entries.is_empty() {
        println!("  (none)");
    } else {
        for e in &entries {
            println!("  {e}");
        }
    }
}

fn subdirs(dir: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

fn workspace_names(home: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(home) else {
        return vec![];
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            let p = e.path();
            p.join("tenants").is_dir() || p.join("orbit.toml").is_file()
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

pub fn find_icase(parent: &Path, name: &str) -> Option<std::path::PathBuf> {
    let needle = name.to_lowercase();
    let Ok(rd) = fs::read_dir(parent) else {
        return None;
    };
    rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.is_dir()
            && p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase() == needle)
                .unwrap_or(false)
    })
}
