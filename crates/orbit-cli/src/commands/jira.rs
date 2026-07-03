use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::{
    jira::{self, JiraIssue, TaskContext, load_orgs},
};
use std::io::{self, Write};

// ── clap types ────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct JiraArgs {
    #[command(subcommand)]
    pub command: Option<JiraCommand>,
}

#[derive(Debug, Subcommand)]
pub enum JiraCommand {
    /// Add a comment to an issue
    Comment {
        /// Issue key (e.g. PROJ-123)
        key: String,
        /// Comment body
        body: String,
    },
    /// Update the description of an issue
    Describe {
        /// Issue key (e.g. PROJ-123)
        key: String,
        /// New description text
        body: String,
    },
}

pub fn run(args: JiraArgs) -> Result<()> {
    match args.command {
        Some(JiraCommand::Comment { key, body }) => {
            jira::add_comment(&key, &body)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("  ✓ Comment added to {key}");
            Ok(())
        }
        Some(JiraCommand::Describe { key, body }) => {
            jira::append_description(&key, &body)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("  ✓ Description appended for {key}");
            Ok(())
        }
        None => run_status(),
    }
}

fn run_status() -> Result<()> {
    let orgs = load_orgs();
    if orgs.is_empty() {
        println!("  Jira · no orgs configured");
        println!("  Add orgs to: ~/.config/orbit/plugins/jira/orgs.toml");
        println!();
        println!("  Example:");
        println!("    [[orgs]]");
        println!("    id  = \"myorg\"");
        println!("    url = \"https://myorg.atlassian.net\"");
        println!();
        println!("  Authenticate: acli jira auth login --site myorg.atlassian.net --email you@example.com --token < api-token.txt");
        return Ok(());
    }
    println!(
        "  Jira · {} org{}",
        orgs.len(),
        if orgs.len() == 1 { "" } else { "s" }
    );
    println!();
    for org in &orgs {
        println!("    {}  ·  {}", org.id, org.url);
    }
    println!();
    println!("  Run `acli jira auth status` to check active account.");
    Ok(())
}

// ── launch integration ────────────────────────────────────────────────────────

/// Called from `launch.rs` after scope resolution.
/// Returns a TaskContext if the user selects a Jira issue, or None to skip.
pub fn resolve_task_for_launch(task_key: Option<&str>, no_task: bool) -> Option<TaskContext> {
    if no_task {
        return None;
    }

    // Explicit key: fetch directly, no plugin-state gate needed
    if let Some(key) = task_key {
        let orgs = load_orgs();
        return fetch_single_issue(key, &orgs);
    }

    // Interactive flow: require plugin to be enabled
    let state = orbit_core::plugin::PluginState::load();
    if !state.is_enabled("jira") {
        return None;
    }

    let orgs = load_orgs();
    if orgs.is_empty() {
        return None;
    }

    // Fetch all assigned issues across all orgs
    println!("  Fetching tasks…");
    let issues = jira::fetch_issues(&orgs);

    if issues.is_empty() {
        println!("  No assigned tasks found.");
        println!();
        return None;
    }

    // Clear the "Fetching…" line
    print!("\x1b[1A\x1b[2K");
    io::stdout().flush().ok();

    println!();
    let idx = pick_issue_inline(&issues)?;
    let task = TaskContext::from(issues.into_iter().nth(idx)?);
    println!("  Task: \x1b[1m{}\x1b[0m  {}", task.key, task.summary);
    println!();
    Some(task)
}

fn fetch_single_issue(key: &str, orgs: &[orbit_core::jira::JiraOrg]) -> Option<TaskContext> {
    use std::process::Command;

    let out = Command::new("acli")
        .args(["jira", "workitem", "view", key, "--json"])
        .output()
        .ok()?;

    if !out.status.success() {
        eprintln!("  acli: could not fetch issue {key}");
        return None;
    }

    let val: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;

    // acli: key is top-level, everything else is under `fields`
    let issue_key = val.get("key").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    if issue_key.is_empty() {
        return None;
    }
    let fields = val.get("fields").unwrap_or(&val);

    let str_field = |k: &str| {
        fields.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string()
    };
    let nested_name = |outer: &str| {
        fields
            .get(outer)
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from)
    };

    // Find org from key prefix (e.g. "SHOP-187" → project "SHOP")
    let project_key = issue_key.split('-').next().unwrap_or("").to_lowercase();
    let org = orgs
        .iter()
        .find(|o| o.id.to_lowercase().contains(&project_key))
        .map(|o| o.id.clone())
        .unwrap_or_default();

    Some(TaskContext {
        key: issue_key,
        summary: str_field("summary"),
        status: nested_name("status").unwrap_or_else(|| str_field("status")),
        priority: nested_name("priority").unwrap_or_else(|| str_field("priority")),
        issue_type: nested_name("issuetype").unwrap_or_default(),
        board_id: String::new(),
        board_name: String::new(),
        org,
    })
}

// ── inline issue picker ───────────────────────────────────────────────────────

fn pick_issue_inline(issues: &[JiraIssue]) -> Option<usize> {
    use crossterm::{
        cursor::MoveUp,
        event::{self, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode},
    };

    let count = issues.len();
    print_issue_rows(issues, 0);
    println!();
    println!("  \x1b[2m[↑↓/jk] nav  [↵] attach  [Esc] skip\x1b[0m");
    io::stdout().flush().ok();

    enable_raw_mode().ok()?;

    let mut selected = 0usize;
    let footer_lines = 2u16;

    let result = loop {
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if selected > 0 {
                    selected -= 1;
                    execute!(io::stdout(), MoveUp(count as u16 + footer_lines)).ok();
                    print_issue_rows(issues, selected);
                    println!();
                    println!("  \x1b[2m[↑↓/jk] nav  [↵] attach  [Esc] skip\x1b[0m");
                    io::stdout().flush().ok();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if selected < count - 1 {
                    selected += 1;
                    execute!(io::stdout(), MoveUp(count as u16 + footer_lines)).ok();
                    print_issue_rows(issues, selected);
                    println!();
                    println!("  \x1b[2m[↑↓/jk] nav  [↵] attach  [Esc] skip\x1b[0m");
                    io::stdout().flush().ok();
                }
            }
            KeyCode::Enter => break Some(selected),
            KeyCode::Esc | KeyCode::Char('q') => break None,
            _ => {}
        }
    };

    disable_raw_mode().ok();

    execute!(io::stdout(), MoveUp(count as u16 + footer_lines)).ok();
    for _ in 0..(count + footer_lines as usize) {
        print!("\x1b[2K\r\n");
    }
    execute!(io::stdout(), MoveUp(count as u16 + footer_lines)).ok();
    io::stdout().flush().ok();

    result
}

fn print_issue_rows(issues: &[JiraIssue], selected: usize) {
    for (i, issue) in issues.iter().enumerate() {
        let (cursor, style, reset) = if i == selected {
            (">", "\x1b[1m", "\x1b[0m")
        } else {
            (" ", "\x1b[2m", "\x1b[0m")
        };
        println!(
            "\r  {} {}{:<12}  {:<44}  {}{reset}",
            cursor,
            style,
            issue.key,
            truncate(&issue.summary, 44),
            issue.status,
        );
    }
}

// ── misc helpers ──────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}
