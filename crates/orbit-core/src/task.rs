use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::data_paths;

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Blocked,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "todo" => Some(Self::Todo),
            "in_progress" | "inprogress" | "in-progress" => Some(Self::InProgress),
            "done" => Some(Self::Done),
            "blocked" => Some(Self::Blocked),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::Done => "Done",
            Self::Blocked => "Blocked",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" | "lowest" | "minor" => Some(Self::Low),
            "medium" | "normal" | "medio" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" | "highest" | "blocker" => Some(Self::Critical),
            _ => None,
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Low => "↓ ",
            Self::Medium => "→ ",
            Self::High => "↑ ",
            Self::Critical => "↑↑",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskSource {
    Manual,
    Plugin {
        name: String,
        external_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

impl TaskSource {
    pub fn label(&self) -> String {
        match self {
            Self::Manual => "manual".into(),
            Self::Plugin { name, .. } => name.clone(),
        }
    }

    pub fn external_id(&self) -> Option<&str> {
        match self {
            Self::Manual => None,
            Self::Plugin { external_id, .. } => Some(external_id.as_str()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitTask {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    pub source: TaskSource,
    /// Canonical workspace name (e.g. "AI", "BeFra")
    pub workspace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

// ── filter + patch ────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub source: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
    pub limit: Option<usize>,
}

impl TaskFilter {
    pub fn matches(&self, task: &OrbitTask) -> bool {
        if let Some(s) = &self.status
            && &task.status != s
        {
            return false;
        }
        if let Some(src) = &self.source
            && task.source.label() != *src
        {
            return false;
        }
        if let Some(t) = &self.tenant
            && task.tenant.as_deref() != Some(t.as_str())
        {
            return false;
        }
        if let Some(p) = &self.project
            && task.project.as_deref() != Some(p.as_str())
        {
            return false;
        }
        if let Some(r) = &self.repository
            && task.repository.as_deref() != Some(r.as_str())
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub task_type: Option<Option<String>>,
    pub tenant: Option<Option<String>>,
    pub project: Option<Option<String>>,
    pub repository: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
}

/// Data carried by `upsert_by_external_id` to stay under the 7-arg limit.
pub struct UpsertData {
    pub title: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub task_type: Option<String>,
    pub url: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
}

// ── ID generation ─────────────────────────────────────────────────────────────

/// Returns the next sequential ID for the given workspace (e.g. "OT-000001").
fn next_id(workspace: &str) -> Result<String> {
    let tasks = load_all(workspace)?;
    let max = tasks
        .iter()
        .filter_map(|t| t.id.strip_prefix("OT-").and_then(|n| n.parse::<u64>().ok()))
        .max()
        .unwrap_or(0);
    Ok(format!("OT-{:06}", max + 1))
}

// ── low-level I/O ─────────────────────────────────────────────────────────────

fn load_all(workspace: &str) -> Result<Vec<OrbitTask>> {
    let path = data_paths::tasks_index_path_for(Some(workspace));
    if !path.exists() {
        return Ok(vec![]);
    }
    let reader = BufReader::new(fs::File::open(&path)?);
    let tasks = reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<OrbitTask>(&l).ok())
        .collect();
    Ok(tasks)
}

fn write_all(workspace: &str, tasks: &[OrbitTask]) -> Result<()> {
    let path = data_paths::tasks_index_path_for(Some(workspace));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&path)?;
    for task in tasks {
        writeln!(file, "{}", serde_json::to_string(task)?)?;
    }
    Ok(())
}

fn append_one(workspace: &str, task: &OrbitTask) -> Result<()> {
    let path = data_paths::tasks_index_path_for(Some(workspace));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", serde_json::to_string(task)?)?;
    Ok(())
}

// ── public API ────────────────────────────────────────────────────────────────

pub fn add(workspace: &str, mut task: OrbitTask) -> Result<OrbitTask> {
    task.id = next_id(workspace)?;
    task.workspace = workspace.to_string();
    let ts = now_secs();
    task.created_at = ts;
    task.updated_at = ts;
    append_one(workspace, &task)?;
    Ok(task)
}

/// Upsert by (plugin name, external_id). If found: updates mutable fields and
/// rewrites the file. If not: assigns a new ID and appends.
pub fn upsert_by_external_id(
    workspace: &str,
    plugin_name: &str,
    external_id: &str,
    data: UpsertData,
) -> Result<OrbitTask> {
    let mut tasks = load_all(workspace)?;
    let now = now_secs();

    let pos = tasks.iter().position(|t| {
        matches!(
            &t.source,
            TaskSource::Plugin { name, external_id: eid, .. }
            if name == plugin_name && eid == external_id
        )
    });

    if let Some(idx) = pos {
        tasks[idx].title = data.title;
        tasks[idx].status = data.status;
        tasks[idx].priority = data.priority;
        tasks[idx].task_type = data.task_type;
        if let TaskSource::Plugin { url: ref mut u, .. } = tasks[idx].source {
            *u = data.url;
        }
        tasks[idx].tenant = data.tenant;
        tasks[idx].project = data.project;
        tasks[idx].repository = data.repository;
        tasks[idx].updated_at = now;
        let result = tasks[idx].clone();
        write_all(workspace, &tasks)?;
        Ok(result)
    } else {
        let id = {
            let max = tasks
                .iter()
                .filter_map(|t| t.id.strip_prefix("OT-").and_then(|n| n.parse::<u64>().ok()))
                .max()
                .unwrap_or(0);
            format!("OT-{:06}", max + 1)
        };
        let task = OrbitTask {
            id,
            title: data.title,
            description: None,
            status: data.status,
            priority: data.priority,
            task_type: data.task_type,
            source: TaskSource::Plugin {
                name: plugin_name.to_string(),
                external_id: external_id.to_string(),
                url: data.url,
            },
            workspace: workspace.to_string(),
            tenant: data.tenant,
            project: data.project,
            repository: data.repository,
            tags: vec![],
            created_at: now,
            updated_at: now,
        };
        append_one(workspace, &task)?;
        Ok(task)
    }
}

/// Returns tasks newest-first (by updated_at).
pub fn list(workspace: &str, filter: &TaskFilter) -> Result<Vec<OrbitTask>> {
    let mut tasks = load_all(workspace)?;
    tasks.retain(|t| filter.matches(t));
    tasks.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
    if let Some(limit) = filter.limit {
        tasks.truncate(limit);
    }
    Ok(tasks)
}

/// Aggregate tasks across all workspace NDJSON files; applies filter after merge.
pub fn list_all_workspaces(filter: &TaskFilter) -> Result<Vec<OrbitTask>> {
    let mut all = Vec::new();
    for path in data_paths::all_tasks_paths() {
        if !path.exists() {
            continue;
        }
        let reader = BufReader::new(fs::File::open(&path)?);
        let tasks: Vec<OrbitTask> = reader
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect();
        all.extend(tasks);
    }
    all.retain(|t| filter.matches(t));
    all.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
    if let Some(limit) = filter.limit {
        all.truncate(limit);
    }
    Ok(all)
}

pub fn get(workspace: &str, id: &str) -> Result<Option<OrbitTask>> {
    let tasks = load_all(workspace)?;
    Ok(tasks.into_iter().find(|t| t.id == id))
}

pub fn update(workspace: &str, id: &str, patch: TaskPatch) -> Result<OrbitTask> {
    let mut tasks = load_all(workspace)?;
    let pos = tasks
        .iter()
        .position(|t| t.id == id)
        .ok_or_else(|| anyhow::anyhow!("task {id} not found in workspace {workspace}"))?;

    let t = &mut tasks[pos];
    if let Some(v) = patch.title {
        t.title = v;
    }
    if let Some(v) = patch.description {
        t.description = v;
    }
    if let Some(v) = patch.status {
        t.status = v;
    }
    if let Some(v) = patch.priority {
        t.priority = v;
    }
    if let Some(v) = patch.task_type {
        t.task_type = v;
    }
    if let Some(v) = patch.tenant {
        t.tenant = v;
    }
    if let Some(v) = patch.project {
        t.project = v;
    }
    if let Some(v) = patch.repository {
        t.repository = v;
    }
    if let Some(v) = patch.tags {
        t.tags = v;
    }
    t.updated_at = now_secs();
    let result = tasks[pos].clone();
    write_all(workspace, &tasks)?;
    Ok(result)
}

/// Returns true if a task was deleted.
pub fn delete(workspace: &str, id: &str) -> Result<bool> {
    let mut tasks = load_all(workspace)?;
    let before = tasks.len();
    tasks.retain(|t| t.id != id);
    if tasks.len() == before {
        return Ok(false);
    }
    write_all(workspace, &tasks)?;
    Ok(true)
}

/// Returns all tasks for the active workspace (for TUI display).
pub fn load_for_tui(workspace: &str) -> Vec<OrbitTask> {
    list(workspace, &TaskFilter::default()).unwrap_or_default()
}

/// mtime of the task store file (used by TUI to detect changes from the daemon).
pub fn store_mtime(workspace: &str) -> Option<std::time::SystemTime> {
    let path = data_paths::tasks_index_path_for(Some(workspace));
    std::fs::metadata(&path).ok()?.modified().ok()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_tmp() -> TempDir {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_DATA_HOME", dir.path().to_str().unwrap());
        }
        dir
    }

    fn make_task(title: &str) -> OrbitTask {
        OrbitTask {
            id: String::new(),
            title: title.into(),
            description: None,
            status: TaskStatus::Todo,
            priority: TaskPriority::Medium,
            task_type: None,
            source: TaskSource::Manual,
            workspace: "AI".into(),
            tenant: None,
            project: None,
            repository: None,
            tags: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn add_and_list() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let _dir = setup_tmp();

        let t = add("AI", make_task("First task")).unwrap();
        assert_eq!(t.id, "OT-000001");

        let t2 = add("AI", make_task("Second task")).unwrap();
        assert_eq!(t2.id, "OT-000002");

        let tasks = list("AI", &TaskFilter::default()).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn update_and_delete() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let _dir = setup_tmp();

        let t = add("AI", make_task("Fix bug")).unwrap();
        let id = t.id.clone();

        let updated = update(
            "AI",
            &id,
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);

        let deleted = delete("AI", &id).unwrap();
        assert!(deleted);

        let tasks = list("AI", &TaskFilter::default()).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn upsert_creates_then_updates() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let _dir = setup_tmp();

        let t1 = upsert_by_external_id(
            "AI",
            "jira",
            "PROJ-1",
            UpsertData {
                title: "Do thing".into(),
                status: TaskStatus::Todo,
                priority: TaskPriority::Medium,
                task_type: None,
                url: None,
                tenant: None,
                project: None,
                repository: None,
            },
        )
        .unwrap();
        assert_eq!(t1.id, "OT-000001");

        let t2 = upsert_by_external_id(
            "AI",
            "jira",
            "PROJ-1",
            UpsertData {
                title: "Do thing updated".into(),
                status: TaskStatus::InProgress,
                priority: TaskPriority::High,
                task_type: None,
                url: None,
                tenant: None,
                project: None,
                repository: None,
            },
        )
        .unwrap();
        assert_eq!(t2.id, "OT-000001");
        assert_eq!(t2.title, "Do thing updated");

        let tasks = list("AI", &TaskFilter::default()).unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn status_parse_roundtrip() {
        assert_eq!(
            TaskStatus::parse("in-progress"),
            Some(TaskStatus::InProgress)
        );
        assert_eq!(
            TaskStatus::parse("in_progress"),
            Some(TaskStatus::InProgress)
        );
        assert_eq!(TaskStatus::parse("todo"), Some(TaskStatus::Todo));
        assert_eq!(TaskStatus::parse("done"), Some(TaskStatus::Done));
        assert_eq!(TaskStatus::parse("unknown"), None);
    }
}
