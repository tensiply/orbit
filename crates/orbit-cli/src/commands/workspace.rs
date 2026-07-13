use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::{
    data_paths::{all_plans_dirs, slugify},
    user_config::UserConfig,
    workspace_registry::WorkspaceRegistry,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub command: WorkspaceCommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Register a workspace so orbit can track plans and memory for it
    Add {
        /// Path to the workspace's AI governance root (e.g. ~/BeFra)
        path: PathBuf,
        /// Short name for the workspace (defaults to the directory name)
        #[arg(long, short)]
        name: Option<String>,
        /// Make this the default workspace
        #[arg(long, short)]
        default: bool,
    },
    /// List all registered workspaces
    List,
    /// Set a workspace as the default
    Default {
        /// Workspace name or slug
        name: String,
    },
    /// Remove a workspace from the registry (does not delete data)
    Remove {
        /// Workspace name or slug
        name: String,
    },
}

pub fn run(args: WorkspaceArgs) -> Result<()> {
    match args.command {
        WorkspaceCommand::Add {
            path,
            name,
            default,
        } => add(path, name, default),
        WorkspaceCommand::List => list(),
        WorkspaceCommand::Default { name } => set_default(name),
        WorkspaceCommand::Remove { name } => remove(name),
    }
}

// ── add ───────────────────────────────────────────────────────────────────────

fn add(path: PathBuf, name: Option<String>, make_default: bool) -> Result<()> {
    let expanded = expand_tilde(&path);

    if !expanded.is_dir() {
        bail!(
            "path does not exist or is not a directory: {}",
            expanded.display()
        );
    }

    let ws_name = name.unwrap_or_else(|| {
        expanded
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string())
    });

    // If this is the first workspace and --default wasn't requested, still
    // make it default — a registry with one workspace should have a default.
    let mut reg = WorkspaceRegistry::load();
    let is_first = reg.workspaces.is_empty();
    let is_default = make_default || is_first;

    reg.add(&ws_name, expanded.clone(), is_default);
    reg.save()?;

    let slug = slugify(&ws_name);
    let default_marker = if is_default { " (default)" } else { "" };
    println!("  Registered workspace: {ws_name}{default_marker}");
    println!("  Slug: {slug}");
    println!("  Path: {}", expanded.display());
    println!("  Plans stored at: ~/.local/share/orbit/workspaces/{slug}/plans/");
    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────────

fn list() -> Result<()> {
    let reg = WorkspaceRegistry::load();
    let user_cfg = UserConfig::load();

    if reg.workspaces.is_empty() {
        println!("  No workspaces registered.");
        println!();
        println!(
            "  The legacy workspace (ai_root: {}) is always active.",
            user_cfg.ai_root_expanded().display()
        );
        println!("  Register additional workspaces with: orbit workspace add <path>");
        return Ok(());
    }

    println!();
    let name_w = reg
        .workspaces
        .iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(4);
    let slug_w = reg
        .workspaces
        .iter()
        .map(|e| e.slug.len())
        .max()
        .unwrap_or(4);

    println!(
        "  {:<name_w$}  {:<slug_w$}  PATH",
        "NAME",
        "SLUG",
        name_w = name_w,
        slug_w = slug_w
    );
    println!(
        "  {:<name_w$}  {:<slug_w$}  {}",
        "─".repeat(name_w),
        "─".repeat(slug_w),
        "─".repeat(40),
        name_w = name_w,
        slug_w = slug_w
    );

    for entry in &reg.workspaces {
        let marker = if entry.is_default { "*" } else { " " };
        println!(
            "  {marker}{:<name_w$}  {:<slug_w$}  {}",
            entry.name,
            entry.slug,
            entry.ai_root.display(),
            name_w = name_w.saturating_sub(1),
            slug_w = slug_w
        );
    }
    println!();
    println!("  * = default workspace");

    // Show plan counts per workspace
    let dirs = all_plans_dirs();
    if dirs.len() > 1 {
        println!();
        println!("  Plan counts:");
        for dir in &dirs {
            let count = count_json_files(dir);
            let label = workspace_label_for_dir(dir, &reg);
            println!("    {label}: {count}");
        }
    }

    println!();
    Ok(())
}

// ── default ───────────────────────────────────────────────────────────────────

fn set_default(name: String) -> Result<()> {
    let mut reg = WorkspaceRegistry::load();
    if !reg.set_default(&name) {
        bail!(
            "workspace '{}' not found. Run `orbit workspace list` to see registered workspaces.",
            name
        );
    }
    reg.save()?;
    println!("  Default workspace set to: {name}");
    Ok(())
}

// ── remove ────────────────────────────────────────────────────────────────────

fn remove(name: String) -> Result<()> {
    let mut reg = WorkspaceRegistry::load();
    if !reg.remove(&name) {
        bail!(
            "workspace '{}' not found. Run `orbit workspace list` to see registered workspaces.",
            name
        );
    }
    reg.save()?;
    println!("  Removed workspace: {name}");
    println!(
        "  (Data in ~/.local/share/orbit/workspaces/{} was not deleted)",
        slugify(&name)
    );
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if (s.starts_with("~/") || s == "~")
        && let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf())
    {
        return home.join(s.trim_start_matches("~/"));
    }
    path.to_path_buf()
}

fn count_json_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_ok_and(|t| t.is_file())
                        && e.path().extension().is_some_and(|x| x == "json")
                })
                .count()
        })
        .unwrap_or(0)
}

fn workspace_label_for_dir(dir: &std::path::Path, reg: &WorkspaceRegistry) -> String {
    // dir is like ~/.local/share/orbit/workspaces/{slug}/plans or ~/.local/share/orbit/plans
    let path_str = dir.to_string_lossy();
    if path_str.contains("/workspaces/") {
        // Extract slug from path
        if let Some(slug) = dir
            .parent() // /workspaces/{slug}
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
        {
            // Find name in registry
            let name = reg
                .workspaces
                .iter()
                .find(|e| e.slug == slug)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| slug.clone());
            return format!("{name} ({slug})");
        }
    }
    "legacy (default ai_root)".to_string()
}
