use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::{
    data_paths::scope_catalog_path,
    scope_catalog::{check_workspaces, ScopeCatalog},
    scope_index::ScopeIndexEntry,
    user_config::UserConfig,
    workspace_registry::{WorkspaceEntry, WorkspaceRegistry},
};

#[derive(Debug, Args)]
pub struct ScopeArgs {
    #[command(subcommand)]
    pub command: ScopeCommand,
}

#[derive(Debug, Subcommand)]
pub enum ScopeCommand {
    /// Scan all registered workspaces and rebuild the scope catalog
    Scan,
    /// List all repositories in the scope catalog
    List {
        /// Filter by workspace name
        #[arg(long, short)]
        workspace: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Validate governance file completeness across all scopes
    Check {
        /// Filter by workspace name (checks all workspaces if omitted)
        #[arg(long, short)]
        workspace: Option<String>,
    },
}

pub fn run(args: ScopeArgs) -> Result<()> {
    match args.command {
        ScopeCommand::Scan => scan(),
        ScopeCommand::List { workspace, json } => list(workspace, json),
        ScopeCommand::Check { workspace } => check(workspace),
    }
}

// ── scan ──────────────────────────────────────────────────────────────────────

fn scan() -> Result<()> {
    let workspaces = all_workspaces();
    println!();
    println!("  Scanning {} workspace(s)...", workspaces.len());
    for ws in &workspaces {
        println!("    {} → {}", ws.name, ws.ai_root.display());
    }
    println!();

    let catalog = ScopeCatalog::scan(&workspaces);
    let count = catalog.entries.len();
    catalog.save()?;

    println!("  \x1b[32m✓\x1b[0m  {count} repositories indexed");
    println!("  Catalog: {}", scope_catalog_path().display());

    let unregistered = hint_unregistered(&workspaces);
    if !unregistered.is_empty() {
        println!();
        println!("  \x1b[2mRegister additional workspaces to include them in the catalog:\x1b[0m");
        for (name, path) in &unregistered {
            println!(
                "  \x1b[2morbit workspace add {} --name {name}\x1b[0m",
                path.display()
            );
        }
    }

    println!();
    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────────

fn list(workspace: Option<String>, as_json: bool) -> Result<()> {
    let entries = load_entries(workspace.as_deref());

    if as_json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!();
        println!("  No repositories found.");
        println!("  Run `orbit scope scan` to build the catalog.");
        println!();
        return Ok(());
    }

    print_table(&entries);
    Ok(())
}

fn print_table(entries: &[ScopeIndexEntry]) {
    let ws_w = entries.iter().map(|e| e.workspace.len()).max().unwrap_or(9).max(9);
    let t_w = entries.iter().map(|e| e.tenant.len()).max().unwrap_or(6).max(6);
    let p_w = entries.iter().map(|e| e.project.len()).max().unwrap_or(7).max(7);
    let r_w = entries.iter().map(|e| e.repository.len()).max().unwrap_or(10).max(10);
    let sep = ws_w + 2 + t_w + 2 + p_w + 2 + r_w + 2 + 40;

    println!();
    println!(
        "  \x1b[2m{ws:<ws_w$}  {t:<t_w$}  {p:<p_w$}  {r:<r_w$}  description\x1b[0m",
        ws = "workspace",
        t = "tenant",
        p = "project",
        r = "repository",
        ws_w = ws_w,
        t_w = t_w,
        p_w = p_w,
        r_w = r_w,
    );
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep));

    let mut last_ws = String::new();
    for entry in entries {
        let ws_label = if entry.workspace != last_ws {
            last_ws = entry.workspace.clone();
            entry.workspace.clone()
        } else {
            " ".repeat(entry.workspace.len())
        };

        let desc = entry.description.as_deref().unwrap_or("");
        let desc = truncate(desc, 38);

        println!(
            "  {ws:<ws_w$}  \x1b[2m{t:<t_w$}\x1b[0m  {p:<p_w$}  {r:<r_w$}  \x1b[2m{desc}\x1b[0m",
            ws = ws_label,
            t = entry.tenant,
            p = entry.project,
            r = entry.repository,
            ws_w = ws_w,
            t_w = t_w,
            p_w = p_w,
            r_w = r_w,
        );
    }

    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep));
    println!();
    println!("  {} repositories", entries.len());
    println!();
}

// ── check ─────────────────────────────────────────────────────────────────────

fn check(workspace: Option<String>) -> Result<()> {
    let mut workspaces = all_workspaces();

    if let Some(ws) = &workspace {
        workspaces.retain(|e| e.name.eq_ignore_ascii_case(ws));
        if workspaces.is_empty() {
            anyhow::bail!(
                "workspace '{}' not found. Run `orbit workspace list` to see registered workspaces.",
                ws
            );
        }
    }

    let issues = check_workspaces(&workspaces);

    if issues.is_empty() {
        println!();
        println!("  \x1b[32m✓\x1b[0m  All scopes are complete.");
        println!();
        return Ok(());
    }

    println!();
    println!("  \x1b[33m{} governance issue(s):\x1b[0m", issues.len());
    println!();

    let scope_w = issues.iter().map(|i| i.scope.len()).max().unwrap_or(10);
    for issue in &issues {
        println!(
            "  \x1b[33m◆\x1b[0m  {:<scope_w$}  \x1b[2m{}\x1b[0m",
            issue.scope,
            issue.issue,
            scope_w = scope_w,
        );
    }

    println!();
    println!("  Run `orbit scope scan` after fixing to update the catalog.");
    println!();
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Returns all workspaces: registered ones + the UserConfig ai_root if not already registered.
fn all_workspaces() -> Vec<WorkspaceEntry> {
    let mut reg = WorkspaceRegistry::load();
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();

    let already_registered = reg.workspaces.iter().any(|e| e.ai_root == ai_root);
    if !already_registered {
        // Use the directory name of ai_root itself ("AI") as the workspace name.
        // Other workspaces (BeFra, Eloir, ...) are explicitly registered via `orbit workspace add`.
        let ws_name = ai_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "AI".to_string());
        reg.workspaces.push(WorkspaceEntry {
            slug: ws_name.to_lowercase(),
            name: ws_name,
            ai_root,
            is_default: true,
        });
    }

    reg.workspaces
}

/// Returns workspaces that exist on disk but are not in the registry.
fn hint_unregistered(registered: &[WorkspaceEntry]) -> Vec<(String, std::path::PathBuf)> {
    use std::fs;
    let Ok(home) = std::env::var("HOME") else {
        return vec![];
    };
    let home = std::path::PathBuf::from(home);
    let mut hints = Vec::new();

    let Ok(entries) = fs::read_dir(&home) else {
        return vec![];
    };

    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let candidate_ai = entry.path().join("AI");
        if !candidate_ai.is_dir() {
            continue;
        }
        // Check it has a tenants or orbit.json — looks like a workspace
        if !candidate_ai.join("tenants").is_dir() && !candidate_ai.join("orbit.json").is_file() {
            continue;
        }
        let already = registered.iter().any(|e| e.ai_root == candidate_ai);
        if !already {
            let name = entry
                .file_name()
                .to_string_lossy()
                .to_string();
            hints.push((name, candidate_ai));
        }
    }

    hints
}

fn load_entries(workspace: Option<&str>) -> Vec<ScopeIndexEntry> {
    match ScopeCatalog::load() {
        Some(mut c) => {
            if let Some(ws) = workspace {
                c.entries.retain(|e| e.workspace.eq_ignore_ascii_case(ws));
            }
            c.entries
        }
        None => {
            // No catalog yet — scan on the fly
            let workspaces = all_workspaces();
            let mut entries = ScopeCatalog::scan(&workspaces).entries;
            if let Some(ws) = workspace {
                entries.retain(|e| e.workspace.eq_ignore_ascii_case(ws));
            }
            entries
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max.saturating_sub(3))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}
