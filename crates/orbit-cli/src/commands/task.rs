use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use orbit_core::{
    jira,
    task::{
        self, OrbitTask, TaskFilter, TaskPatch, TaskPriority, TaskSource, TaskStatus, UpsertData,
    },
    user_config::UserConfig,
};
use std::path::Path;

#[derive(Debug, Parser)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub subcommand: TaskSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TaskSubcommand {
    /// List tasks (newest first)
    List {
        /// Show tasks from all workspaces
        #[arg(long)]
        all: bool,

        /// Filter by workspace name
        #[arg(long)]
        workspace: Option<String>,

        /// Filter by tenant
        #[arg(long)]
        tenant: Option<String>,

        /// Filter by project
        #[arg(long)]
        project: Option<String>,

        /// Filter by repository
        #[arg(long)]
        repo: Option<String>,

        /// Filter by status (todo, in-progress, done, blocked, cancelled)
        #[arg(long)]
        status: Option<String>,

        /// Filter by source plugin name (e.g. "jira")
        #[arg(long)]
        source: Option<String>,

        /// Max tasks to show
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Create a new manual task
    Add {
        /// Task title
        title: String,

        /// Task description
        #[arg(long)]
        description: Option<String>,

        /// Priority: low, medium, high, critical
        #[arg(long, default_value = "medium")]
        priority: String,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Workspace override (defaults to env)
        #[arg(long)]
        workspace: Option<String>,

        /// Tenant override
        #[arg(long)]
        tenant: Option<String>,

        /// Project override
        #[arg(long)]
        project: Option<String>,

        /// Repository override
        #[arg(long)]
        repo: Option<String>,
    },

    /// Show details of a task
    Get {
        /// Task ID (e.g. OT-000001)
        id: String,

        /// Workspace name
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Update a task's fields
    Update {
        /// Task ID (e.g. OT-000001)
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New status (todo, in-progress, done, blocked, cancelled)
        #[arg(long)]
        status: Option<String>,

        /// New priority (low, medium, high, critical)
        #[arg(long)]
        priority: Option<String>,

        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Workspace override
        #[arg(long)]
        workspace: Option<String>,

        /// Reassign to tenant
        #[arg(long)]
        tenant: Option<String>,

        /// Reassign to project
        #[arg(long)]
        project: Option<String>,

        /// Reassign to repo
        #[arg(long)]
        repo: Option<String>,
    },

    /// Delete a task
    Delete {
        /// Task ID (e.g. OT-000001)
        id: String,

        /// Workspace name
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Import a specific Jira issue as an OrbitTask
    Import {
        /// Source plugin (currently only "jira")
        source: String,

        /// External issue key (e.g. PROJ-123)
        key: String,

        /// Workspace override
        #[arg(long)]
        workspace: Option<String>,

        /// Tenant override
        #[arg(long)]
        tenant: Option<String>,

        /// Project override
        #[arg(long)]
        project: Option<String>,

        /// Repository override
        #[arg(long)]
        repo: Option<String>,
    },
}

// ── scope resolution ──────────────────────────────────────────────────────────

fn resolve_workspace(arg: Option<String>) -> String {
    if let Some(ws) = arg {
        return ws;
    }
    if let Ok(root) = std::env::var("AI_WORKSPACE_ROOT")
        && let Some(name) = Path::new(&root).file_name()
    {
        return name.to_string_lossy().into_owned();
    }
    let root = UserConfig::load().ai_root_expanded();
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AI".into())
}

fn resolve_opt(arg: Option<String>, env_key: &str) -> Option<String> {
    if arg.is_some() {
        return arg;
    }
    let v = std::env::var(env_key).unwrap_or_default();
    if v.is_empty() { None } else { Some(v) }
}

// ── display ───────────────────────────────────────────────────────────────────

fn print_task(t: &OrbitTask) {
    let src = t.source.label();
    let ext = t
        .source
        .external_id()
        .map(|id| format!(" ({id})"))
        .unwrap_or_default();

    let scope_parts: Vec<&str> = [
        Some(t.workspace.as_str()),
        t.tenant.as_deref(),
        t.project.as_deref(),
        t.repository.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let scope = scope_parts.join("/");

    println!(
        "{} [{}] {} | {} | {} | {}{}",
        t.id,
        t.priority.symbol().trim(),
        t.status.display(),
        scope,
        t.title,
        src,
        ext
    );
}

fn print_tasks_table(tasks: &[OrbitTask]) {
    if tasks.is_empty() {
        println!("No tasks found.");
        return;
    }
    println!(
        "{:<12} {:<4} {:<13} {:<30} {:<12} TITLE",
        "ID", "PRI", "STATUS", "SCOPE", "SOURCE"
    );
    println!("{}", "─".repeat(90));
    for t in tasks {
        let scope_parts: Vec<&str> = [
            Some(t.workspace.as_str()),
            t.tenant.as_deref(),
            t.project.as_deref(),
            t.repository.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let scope = scope_parts.join("/");
        let scope_display = if scope.len() > 28 {
            format!("…{}", &scope[scope.len() - 27..])
        } else {
            scope
        };

        let ext = t
            .source
            .external_id()
            .map(|id| format!(" ({})", id))
            .unwrap_or_default();

        println!(
            "{:<12} {:<4} {:<13} {:<30} {:<12} {}{}",
            t.id,
            t.priority.symbol().trim(),
            t.status.display(),
            scope_display,
            t.source.label(),
            t.title,
            ext
        );
    }
}

// ── run ───────────────────────────────────────────────────────────────────────

pub fn run(args: TaskArgs) -> Result<()> {
    match args.subcommand {
        TaskSubcommand::List {
            all,
            workspace,
            tenant,
            project,
            repo,
            status,
            source,
            limit,
        } => {
            let filter = TaskFilter {
                status: status.as_deref().and_then(TaskStatus::parse),
                source,
                tenant,
                project,
                repository: repo,
                limit: Some(limit),
            };

            if all {
                let tasks = task::list_all_workspaces(&filter)?;
                print_tasks_table(&tasks);
            } else {
                let ws = resolve_workspace(workspace);
                let tasks = task::list(&ws, &filter)?;
                print_tasks_table(&tasks);
            }
        }

        TaskSubcommand::Add {
            title,
            description,
            priority,
            tags,
            workspace,
            tenant,
            project,
            repo,
        } => {
            let ws = resolve_workspace(workspace);
            let tenant = resolve_opt(tenant, "AI_TENANT");
            let project = resolve_opt(project, "AI_PROJECT");
            let repository = resolve_opt(repo, "AI_REPOSITORY");

            let pri = TaskPriority::parse(&priority)
                .ok_or_else(|| anyhow::anyhow!("unknown priority: {priority}"))?;

            let tag_vec: Vec<String> = tags
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let orbit_task = OrbitTask {
                id: String::new(),
                title,
                description,
                status: TaskStatus::Todo,
                priority: pri,
                task_type: None,
                source: TaskSource::Manual,
                workspace: ws.clone(),
                tenant,
                project,
                repository,
                tags: tag_vec,
                created_at: 0,
                updated_at: 0,
            };

            let created = task::add(&ws, orbit_task)?;
            println!("Created {}", created.id);
            print_task(&created);
        }

        TaskSubcommand::Get { id, workspace } => {
            let ws = resolve_workspace(workspace);
            match task::get(&ws, &id)? {
                Some(t) => {
                    println!("ID:          {}", t.id);
                    println!("Title:       {}", t.title);
                    println!("Status:      {}", t.status.display());
                    println!("Priority:    {}", t.priority.display());
                    if let Some(d) = &t.description {
                        println!("Description: {d}");
                    }
                    if let Some(tt) = &t.task_type {
                        println!("Type:        {tt}");
                    }
                    println!("Source:      {}{}", t.source.label(), {
                        t.source
                            .external_id()
                            .map(|id| format!(" ({id})"))
                            .unwrap_or_default()
                    });
                    let scope_parts: Vec<&str> = [
                        Some(t.workspace.as_str()),
                        t.tenant.as_deref(),
                        t.project.as_deref(),
                        t.repository.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    println!("Scope:       {}", scope_parts.join("/"));
                    if !t.tags.is_empty() {
                        println!("Tags:        {}", t.tags.join(", "));
                    }
                    if let TaskSource::Plugin { url: Some(url), .. } = &t.source {
                        println!("URL:         {url}");
                    }
                }
                None => bail!("task {id} not found in workspace {ws}"),
            }
        }

        TaskSubcommand::Update {
            id,
            title,
            status,
            priority,
            tags,
            workspace,
            tenant,
            project,
            repo,
        } => {
            let ws = resolve_workspace(workspace);

            let status_val = status
                .as_deref()
                .map(|s| TaskStatus::parse(s).ok_or_else(|| anyhow::anyhow!("unknown status: {s}")))
                .transpose()?;

            let priority_val = priority
                .as_deref()
                .map(|p| {
                    TaskPriority::parse(p).ok_or_else(|| anyhow::anyhow!("unknown priority: {p}"))
                })
                .transpose()?;

            let tag_vec = tags.map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            });

            let patch = TaskPatch {
                title,
                status: status_val,
                priority: priority_val,
                tags: tag_vec,
                tenant: tenant.map(Some),
                project: project.map(Some),
                repository: repo.map(Some),
                ..Default::default()
            };

            let updated = task::update(&ws, &id, patch)?;
            println!("Updated {}", updated.id);
            print_task(&updated);
        }

        TaskSubcommand::Delete { id, workspace } => {
            let ws = resolve_workspace(workspace);
            if task::delete(&ws, &id)? {
                println!("Deleted {id}");
            } else {
                bail!("task {id} not found in workspace {ws}");
            }
        }

        TaskSubcommand::Import {
            source,
            key,
            workspace,
            tenant,
            project,
            repo,
        } => {
            if source != "jira" {
                bail!("only 'jira' is supported as import source right now");
            }
            let ws = resolve_workspace(workspace);
            let tenant = resolve_opt(tenant, "AI_TENANT");
            let project = resolve_opt(project, "AI_PROJECT");
            let repository = resolve_opt(repo, "AI_REPOSITORY");

            // Look in the task store first (cached by poller).
            let existing = task::get(&ws, &key).ok().flatten();
            if let Some(t) = existing {
                println!("Already imported as {}", t.id);
                print_task(&t);
                return Ok(());
            }

            // Otherwise call Jira directly.
            let orgs = jira::load_orgs();
            if orgs.is_empty() {
                bail!("no Jira orgs configured — run `orbit jira auth` first");
            }
            let issues = jira::fetch_issues(&orgs);
            let issue = issues
                .iter()
                .find(|i| i.key == key)
                .ok_or_else(|| anyhow::anyhow!("issue {key} not found in Jira"))?;

            let orbit_task = issue.to_orbit_task(&ws, tenant, project, repository);

            let url = match orbit_task.source {
                TaskSource::Plugin { url, .. } => url,
                _ => None,
            };
            let created = task::upsert_by_external_id(
                &ws,
                "jira",
                &issue.key,
                UpsertData {
                    title: orbit_task.title,
                    status: orbit_task.status,
                    priority: orbit_task.priority,
                    task_type: orbit_task.task_type,
                    url,
                    tenant: orbit_task.tenant,
                    project: orbit_task.project,
                    repository: orbit_task.repository,
                },
            )?;

            println!("Imported {} → {}", key, created.id);
            print_task(&created);
        }
    }
    Ok(())
}
