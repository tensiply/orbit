use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use orbit_core::{
    activity::{self, ActivityEntry},
    user_config::UserConfig,
};
use std::path::Path;

use crate::output::truncate_desc;

#[derive(Debug, Parser)]
pub struct ActivityArgs {
    #[command(subcommand)]
    pub subcommand: ActivitySubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ActivitySubcommand {
    /// List recent activity entries (newest first)
    List {
        /// Filter by scope (partial, case-insensitive)
        #[arg(long)]
        scope: Option<String>,

        /// Filter by session ID
        #[arg(long)]
        session_id: Option<String>,

        /// Max entries to show
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Workspace name (defaults to AI_WORKSPACE_ROOT or ai_root)
        #[arg(long)]
        workspace: Option<String>,

        /// Output as markdown for context injection
        #[arg(long)]
        md: bool,
    },
    /// Append a new activity entry to the log
    Append {
        /// Scope key (e.g. "AIDEV/AI-ECOSYSTEM/orbit"); defaults to AI_TENANT/PROJECT/REPOSITORY env
        #[arg(long)]
        scope: Option<String>,

        /// Activity summary (1-3 lines)
        #[arg(long)]
        summary: Option<String>,

        /// Associate with a session ID to prevent duplicate entries
        #[arg(long)]
        session_id: Option<String>,

        /// Workspace name
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Exit 0 if a session already has an activity entry; 1 otherwise
    Has {
        /// Session ID to check
        #[arg(long)]
        session_id: String,

        /// Workspace name
        #[arg(long)]
        workspace: Option<String>,
    },
}

// ── scope resolution ──────────────────────────────────────────────────────────

fn resolve_workspace(arg: Option<String>) -> Option<String> {
    if let Some(ws) = arg {
        return Some(ws);
    }
    if let Ok(root) = std::env::var("AI_WORKSPACE_ROOT")
        && let Some(name) = Path::new(&root).file_name()
    {
        return Some(name.to_string_lossy().into_owned());
    }
    let root = UserConfig::load().ai_root_expanded();
    root.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn resolve_scope(arg: Option<String>) -> String {
    if let Some(s) = arg {
        return s;
    }
    let tenant = std::env::var("AI_TENANT").unwrap_or_default();
    let project = std::env::var("AI_PROJECT").unwrap_or_default();
    let repository = std::env::var("AI_REPOSITORY").unwrap_or_default();
    activity::scope_key(
        if tenant.is_empty() { None } else { Some(tenant.as_str()) },
        if project.is_empty() { None } else { Some(project.as_str()) },
        if repository.is_empty() { None } else { Some(repository.as_str()) },
    )
}

// ── display ───────────────────────────────────────────────────────────────────

fn print_activity_table(entries: &[ActivityEntry]) {
    println!("activity\n");
    println!("  \x1b[2mSession activity history per scope.\x1b[0m\n");

    if entries.is_empty() {
        println!("  No activity recorded yet.");
        return;
    }

    let ts_w = 16usize;
    let scope_w = entries.iter().map(|e| e.scope.len()).max().unwrap_or(20).max(5);
    let summary_w: usize = 55;
    let sep_w = 2 + ts_w + 2 + scope_w + 2 + summary_w;

    println!(
        "  \x1b[2m{ts:<ts_w$}  {sc:<scope_w$}  summary\x1b[0m",
        ts = "timestamp",
        sc = "scope",
        ts_w = ts_w,
        scope_w = scope_w,
    );
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));

    for e in entries {
        let first_line = e.summary.lines().next().unwrap_or(&e.summary);
        let summary = truncate_desc(first_line, summary_w);
        println!(
            "  {ts:<ts_w$}  {sc:<scope_w$}  {summary}",
            ts = activity::format_ts(e.ts),
            sc = e.scope,
            ts_w = ts_w,
            scope_w = scope_w,
        );
    }

    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));
    println!();

    let n = entries.len();
    let plural = if n == 1 { "entry" } else { "entries" };
    println!("  {n} {plural}  ·  orbit activity append --scope <scope>");
}

// ── run ───────────────────────────────────────────────────────────────────────

pub fn run(args: ActivityArgs) -> Result<()> {
    match args.subcommand {
        ActivitySubcommand::List {
            scope,
            session_id,
            limit,
            workspace,
            md,
        } => {
            let ws = resolve_workspace(workspace);
            let entries = activity::list(
                ws.as_deref(),
                scope.as_deref(),
                session_id.as_deref(),
                limit,
            )?;
            if md {
                print!("{}", activity::format_for_context(&entries));
            } else {
                print_activity_table(&entries);
            }
        }

        ActivitySubcommand::Append {
            scope,
            summary,
            session_id,
            workspace,
        } => {
            let ws = resolve_workspace(workspace);
            let scope_key = resolve_scope(scope);
            if scope_key.is_empty() {
                bail!(
                    "--scope is required (or set AI_TENANT / AI_PROJECT / AI_REPOSITORY env vars)"
                );
            }
            let summary = summary.unwrap_or_else(|| "Session ended".into());
            let mut entry = ActivityEntry::new(scope_key.clone(), summary);
            if let Some(sid) = session_id {
                entry = entry.with_session(sid);
            }
            activity::append(ws.as_deref(), &entry)?;
            println!("Appended entry to {scope_key}");
        }

        ActivitySubcommand::Has {
            session_id,
            workspace,
        } => {
            let ws = resolve_workspace(workspace);
            if !activity::session_exists(ws.as_deref(), &session_id) {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
