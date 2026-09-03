use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::{
    data_paths::scope_catalog_path,
    scope_catalog::{ScopeCatalog, check_workspaces},
    scope_index::ScopeIndexEntry,
    user_config::UserConfig,
    workspace_registry::{WorkspaceEntry, WorkspaceRegistry},
};
use std::path::PathBuf;

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
    /// Create a new scope directory (tenant, project, or repo)
    Create {
        #[command(subcommand)]
        level: ScopeCreateLevel,
    },
    /// Initialize governance files for an existing scope (orbit.json, source-of-truth/)
    Init {
        #[command(subcommand)]
        level: ScopeInitLevel,
    },
}

#[derive(Debug, Subcommand)]
pub enum ScopeCreateLevel {
    /// Create a new tenant directory in the governance structure
    Tenant {
        /// Tenant name (e.g. MYTEAM)
        name: String,
        /// Target workspace (defaults to current/default workspace)
        #[arg(long, short)]
        workspace: Option<String>,
    },
    /// Create a new project directory under a tenant
    Project {
        /// Project name (e.g. BACKEND)
        name: String,
        /// Parent tenant name
        #[arg(long, short)]
        tenant: String,
        /// Target workspace (defaults to current/default workspace)
        #[arg(long, short)]
        workspace: Option<String>,
    },
    /// Create a new repository directory under a project, optionally cloning a git repo
    Repo {
        /// Repository name
        name: String,
        /// Parent project name
        #[arg(long, short)]
        project: String,
        /// Parent tenant name
        #[arg(long, short)]
        tenant: String,
        /// Target workspace (defaults to current/default workspace)
        #[arg(long, short)]
        workspace: Option<String>,
        /// Git URL to clone into the code directory (e.g. git@github.com:org/repo.git)
        #[arg(long)]
        url: Option<String>,
        /// Override the default code clone destination path
        #[arg(long)]
        code_path: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ScopeInitLevel {
    /// Initialize governance files for a tenant (orbit.json + source-of-truth/README.md)
    Tenant {
        /// Tenant name to initialize
        name: String,
        /// Target workspace (defaults to current/default workspace)
        #[arg(long, short)]
        workspace: Option<String>,
    },
    /// Initialize governance files for a project (orbit.json + source-of-truth/README.md)
    Project {
        /// Project name to initialize
        name: String,
        /// Parent tenant name
        #[arg(long, short)]
        tenant: String,
        /// Target workspace (defaults to current/default workspace)
        #[arg(long, short)]
        workspace: Option<String>,
    },
    /// Initialize governance files for a repo (orbit.json + source-of-truth/README.md + conventions.md)
    Repo {
        /// Repository name to initialize
        name: String,
        /// Parent project name
        #[arg(long, short)]
        project: String,
        /// Parent tenant name
        #[arg(long, short)]
        tenant: String,
        /// Target workspace (defaults to current/default workspace)
        #[arg(long, short)]
        workspace: Option<String>,
    },
}

pub fn run(args: ScopeArgs) -> Result<()> {
    match args.command {
        ScopeCommand::Scan => scan(),
        ScopeCommand::List { workspace, json } => list(workspace, json),
        ScopeCommand::Check { workspace } => check(workspace),
        ScopeCommand::Create { level } => create(level),
        ScopeCommand::Init { level } => init(level),
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
    let ws_w = entries
        .iter()
        .map(|e| e.workspace.len())
        .max()
        .unwrap_or(9)
        .max(9);
    let t_w = entries
        .iter()
        .map(|e| e.tenant.len())
        .max()
        .unwrap_or(6)
        .max(6);
    let p_w = entries
        .iter()
        .map(|e| e.project.len())
        .max()
        .unwrap_or(7)
        .max(7);
    let r_w = entries
        .iter()
        .map(|e| e.repository.len())
        .max()
        .unwrap_or(10)
        .max(10);
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
            let name = entry.file_name().to_string_lossy().to_string();
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

// ── create ────────────────────────────────────────────────────────────────────

fn create(level: ScopeCreateLevel) -> Result<()> {
    match level {
        ScopeCreateLevel::Tenant { name, workspace } => create_tenant(&name, workspace.as_deref()),
        ScopeCreateLevel::Project {
            name,
            tenant,
            workspace,
        } => create_project(&name, &tenant, workspace.as_deref()),
        ScopeCreateLevel::Repo {
            name,
            project,
            tenant,
            workspace,
            url,
            code_path,
        } => create_repo(
            &name,
            &project,
            &tenant,
            workspace.as_deref(),
            url,
            code_path,
        ),
    }
}

fn create_tenant(name: &str, workspace: Option<&str>) -> Result<()> {
    let (ai_root, _ws_root) = resolve_ai_root(workspace)?;
    let tenant_dir = ai_root.join("tenants").join(name);

    if tenant_dir.exists() {
        bail!(
            "tenant '{}' already exists at {}",
            name,
            tenant_dir.display()
        );
    }

    std::fs::create_dir_all(&tenant_dir)?;

    println!();
    println!("  \x1b[32m✓\x1b[0m  Tenant created: {name}");
    println!("  Path: {}", tenant_dir.display());
    println!();
    println!(
        "  Next: \x1b[2morbit scope init tenant {name}{ws}\x1b[0m",
        ws = workspace
            .map(|w| format!(" --workspace {w}"))
            .unwrap_or_default()
    );
    println!();
    Ok(())
}

fn create_project(name: &str, tenant: &str, workspace: Option<&str>) -> Result<()> {
    let (ai_root, _ws_root) = resolve_ai_root(workspace)?;
    let project_dir = ai_root
        .join("tenants")
        .join(tenant)
        .join("projects")
        .join(name);

    if project_dir.exists() {
        bail!(
            "project '{}' already exists at {}",
            name,
            project_dir.display()
        );
    }

    // Ensure the tenant exists
    let tenant_dir = ai_root.join("tenants").join(tenant);
    if !tenant_dir.is_dir() {
        bail!(
            "tenant '{}' not found. Create it first with:\n  orbit scope create tenant {tenant}",
            tenant
        );
    }

    std::fs::create_dir_all(&project_dir)?;

    println!();
    println!("  \x1b[32m✓\x1b[0m  Project created: {name}  (tenant: {tenant})");
    println!("  Path: {}", project_dir.display());
    println!();
    println!(
        "  Next: \x1b[2morbit scope init project {name} --tenant {tenant}{ws}\x1b[0m",
        ws = workspace
            .map(|w| format!(" --workspace {w}"))
            .unwrap_or_default()
    );
    println!();
    Ok(())
}

fn create_repo(
    name: &str,
    project: &str,
    tenant: &str,
    workspace: Option<&str>,
    url: Option<String>,
    code_path_override: Option<PathBuf>,
) -> Result<()> {
    let (ai_root, ws_root) = resolve_ai_root(workspace)?;

    // Validate parent scope exists
    let project_dir = ai_root
        .join("tenants")
        .join(tenant)
        .join("projects")
        .join(project);
    if !project_dir.is_dir() {
        bail!(
            "project '{}' not found under tenant '{}'. Create it first with:\n  orbit scope create project {project} --tenant {tenant}",
            project,
            tenant
        );
    }

    let repo_gov_dir = project_dir.join("repositories").join(name);
    if repo_gov_dir.exists() {
        bail!(
            "repo '{}' already exists at {}",
            name,
            repo_gov_dir.display()
        );
    }

    std::fs::create_dir_all(&repo_gov_dir)?;

    println!();
    println!("  \x1b[32m✓\x1b[0m  Repo created: {name}  (tenant: {tenant} / project: {project})");
    println!("  Governance: {}", repo_gov_dir.display());

    if let Some(git_url) = &url {
        let code_dir =
            code_path_override.unwrap_or_else(|| ws_root.join(tenant).join(project).join(name));

        println!("  Cloning {} → {}", git_url, code_dir.display());
        println!();

        let status = std::process::Command::new("git")
            .args(["clone", git_url, &code_dir.to_string_lossy()])
            .status()?;

        if !status.success() {
            bail!("git clone failed — check the URL and your network/SSH access");
        }

        println!("  \x1b[32m✓\x1b[0m  Code cloned to {}", code_dir.display());
    }

    let ws_flag = workspace
        .map(|w| format!(" --workspace {w}"))
        .unwrap_or_default();
    println!();
    println!(
        "  Next: \x1b[2morbit scope init repo {name} --project {project} --tenant {tenant}{ws_flag}\x1b[0m"
    );
    println!();
    Ok(())
}

// ── init ──────────────────────────────────────────────────────────────────────

fn init(level: ScopeInitLevel) -> Result<()> {
    match level {
        ScopeInitLevel::Tenant { name, workspace } => init_tenant(&name, workspace.as_deref()),
        ScopeInitLevel::Project {
            name,
            tenant,
            workspace,
        } => init_project(&name, &tenant, workspace.as_deref()),
        ScopeInitLevel::Repo {
            name,
            project,
            tenant,
            workspace,
        } => init_repo(&name, &project, &tenant, workspace.as_deref()),
    }
}

fn init_tenant(name: &str, workspace: Option<&str>) -> Result<()> {
    let (ai_root, _ws_root) = resolve_ai_root(workspace)?;
    let tenant_dir = ai_root.join("tenants").join(name);

    if !tenant_dir.is_dir() {
        bail!(
            "tenant directory not found: {}\nCreate it first with: orbit scope create tenant {name}",
            tenant_dir.display()
        );
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    let orbit_json = tenant_dir.join("orbit.json");
    if !orbit_json.exists() {
        std::fs::write(
            &orbit_json,
            "{\n  \"instructions\": [\n    \"./source-of-truth/README.md\"\n  ]\n}\n",
        )?;
        created.push("orbit.json");
    } else {
        skipped.push("orbit.json");
    }

    let sot_dir = tenant_dir.join("source-of-truth");
    std::fs::create_dir_all(&sot_dir)?;

    let readme = sot_dir.join("README.md");
    if !readme.exists() {
        std::fs::write(
            &readme,
            format!(
                "# {name}\n\n\
                 ## Misión\n\n\
                 <!-- Una línea: qué problema resuelve este tenant -->\n\n\
                 ## Principios\n\n\
                 <!-- Decisiones de diseño que aplican a todos los proyectos del tenant -->\n\n\
                 ## Proyectos\n\n\
                 <!-- Lista de proyectos bajo este tenant -->\n\n\
                 ## Owner\n\n\
                 <!-- email o alias -->\n"
            ),
        )?;
        created.push("source-of-truth/README.md");
    } else {
        skipped.push("source-of-truth/README.md");
    }

    print_init_result(name, &created, &skipped);
    Ok(())
}

fn init_project(name: &str, tenant: &str, workspace: Option<&str>) -> Result<()> {
    let (ai_root, _ws_root) = resolve_ai_root(workspace)?;
    let project_dir = ai_root
        .join("tenants")
        .join(tenant)
        .join("projects")
        .join(name);

    if !project_dir.is_dir() {
        bail!(
            "project directory not found: {}\nCreate it first with: orbit scope create project {name} --tenant {tenant}",
            project_dir.display()
        );
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    let orbit_json = project_dir.join("orbit.json");
    if !orbit_json.exists() {
        std::fs::write(
            &orbit_json,
            "{\n  \"instructions\": [\n    \"./source-of-truth/README.md\"\n  ]\n}\n",
        )?;
        created.push("orbit.json");
    } else {
        skipped.push("orbit.json");
    }

    let sot_dir = project_dir.join("source-of-truth");
    std::fs::create_dir_all(&sot_dir)?;

    let readme = sot_dir.join("README.md");
    if !readme.exists() {
        std::fs::write(
            &readme,
            format!(
                "# {name}\n\n\
                 ## Visión\n\n\
                 <!-- Qué resuelve este proyecto y para quién -->\n\n\
                 ## Repositorios\n\n\
                 | Repositorio | Estado | Descripción |\n\
                 |---|---|---|\n\
                 | `` | activo | <!-- descripción --> |\n\n\
                 ## Arquitectura\n\n\
                 <!-- Diagrama o descripción de componentes si aplica -->\n\n\
                 ## Decisiones de diseño\n\n\
                 <!-- Decisiones que aplican a todos los repos del proyecto -->\n"
            ),
        )?;
        created.push("source-of-truth/README.md");
    } else {
        skipped.push("source-of-truth/README.md");
    }

    print_init_result(name, &created, &skipped);
    Ok(())
}

fn init_repo(name: &str, project: &str, tenant: &str, workspace: Option<&str>) -> Result<()> {
    let (ai_root, _ws_root) = resolve_ai_root(workspace)?;
    let repo_dir = ai_root
        .join("tenants")
        .join(tenant)
        .join("projects")
        .join(project)
        .join("repositories")
        .join(name);

    if !repo_dir.is_dir() {
        bail!(
            "repo directory not found: {}\nCreate it first with: orbit scope create repo {name} --project {project} --tenant {tenant}",
            repo_dir.display()
        );
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    let orbit_json = repo_dir.join("orbit.json");
    if !orbit_json.exists() {
        std::fs::write(
            &orbit_json,
            "{\n  \"instructions\": [\n    \"./source-of-truth/README.md\",\n    \"./source-of-truth/conventions.md\"\n  ]\n}\n",
        )?;
        created.push("orbit.json");
    } else {
        skipped.push("orbit.json");
    }

    let sot_dir = repo_dir.join("source-of-truth");
    std::fs::create_dir_all(&sot_dir)?;

    let readme = sot_dir.join("README.md");
    if !readme.exists() {
        std::fs::write(
            &readme,
            format!(
                "# {name}\n\n\
                 ## Propósito\n\n\
                 <!-- Qué hace este repositorio, qué problema resuelve -->\n\n\
                 ## Estado\n\n\
                 <!-- activo / en desarrollo / deprecado -->\n\n\
                 ## Tech stack\n\n\
                 <!-- Lenguaje, framework, runtime principal -->\n\n\
                 ## Estructura\n\n\
                 <!-- Directorios o módulos clave -->\n\n\
                 ## Comandos\n\n\
                 | Comando | Propósito |\n\
                 |---|---|\n\
                 | `` | build / run / test |\n\n\
                 ## Dependencias externas\n\n\
                 <!-- APIs, bases de datos, servicios que consume -->\n"
            ),
        )?;
        created.push("source-of-truth/README.md");
    } else {
        skipped.push("source-of-truth/README.md");
    }

    let conventions = sot_dir.join("conventions.md");
    if !conventions.exists() {
        std::fs::write(
            &conventions,
            format!(
                "# {name} — Conventions\n\n\
                 ## Stack y versiones\n\n\
                 <!-- lenguaje, runtime, framework principales -->\n\n\
                 ## Patrones de código\n\n\
                 <!-- naming, estructura de módulos, patrones preferidos -->\n\n\
                 ## Error handling\n\n\
                 <!-- cómo se manejan errores en este codebase -->\n\n\
                 ## Testing\n\n\
                 <!-- approach: unit / integration / e2e, herramientas, patrones -->\n\n\
                 ## Commits\n\n\
                 <!-- scope convencional para este repo (ver git-commit.md) -->\n\n\
                 ## CI gates\n\n\
                 <!-- qué debe pasar antes de merge -->\n"
            ),
        )?;
        created.push("source-of-truth/conventions.md");
    } else {
        skipped.push("source-of-truth/conventions.md");
    }

    print_init_result(name, &created, &skipped);
    Ok(())
}

fn print_init_result(name: &str, created: &[&str], skipped: &[&str]) {
    println!();
    if created.is_empty() {
        println!("  \x1b[33m⚠\x1b[0m  {name}: all files already exist");
    } else {
        println!("  \x1b[32m✓\x1b[0m  {name}: governance initialized");
        for f in created {
            println!("      \x1b[32m+\x1b[0m {f}");
        }
        for f in skipped {
            println!("      \x1b[2m~ {f} (already exists)\x1b[0m");
        }
    }
    println!();
    println!("  Run `orbit scope scan` to update the catalog.");
    println!();
}

// ── workspace resolution ──────────────────────────────────────────────────────

/// Resolves (ai_root, workspace_root) for the given workspace name.
/// Uses the default workspace when name is None.
fn resolve_ai_root(workspace: Option<&str>) -> Result<(PathBuf, PathBuf)> {
    let workspaces = all_workspaces();

    let entry = if let Some(ws) = workspace {
        workspaces
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(ws) || e.slug.eq_ignore_ascii_case(ws))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace '{}' not found. Run `orbit workspace list` to see registered workspaces.",
                    ws
                )
            })?
    } else {
        workspaces
            .iter()
            .find(|e| e.is_default)
            .or_else(|| workspaces.first())
            .ok_or_else(|| anyhow::anyhow!("no workspace configured. Run `orbit setup`."))?
    };

    let ai_root = entry.ai_root.clone();
    // The workspace root is the parent of the AI governance dir.
    // e.g. ai_root = ~/Tensiply/AI → workspace_root = ~/Tensiply
    // e.g. ai_root = ~/AI → workspace_root = ~/ (home)
    let ws_root = ai_root
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| ai_root.clone());

    Ok((ai_root, ws_root))
}
