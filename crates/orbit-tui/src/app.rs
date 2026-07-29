use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use orbit_core::{
    engine::Engine, ipc::socket_path, plan::Plan, schedule::ScheduledPlan, session::Session,
    user_config::UserConfig, workspace_config::detect_workspaces,
};
use ratatui::{Terminal, backend::CrosstermBackend, widgets::TableState};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{LaunchParams, views::chat, widget::TextInput};

// ── tabs ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    Sessions,
    Launch,
    Plans,
    System,
    Tasks,
    Schedules,
    Scopes,
    Workspaces,
    Peers,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Tab::Chat => Tab::Sessions,
            Tab::Sessions => Tab::Launch,
            Tab::Launch => Tab::Plans,
            Tab::Plans => Tab::System,
            Tab::System => Tab::Tasks,
            Tab::Tasks => Tab::Schedules,
            Tab::Schedules => Tab::Scopes,
            Tab::Scopes => Tab::Workspaces,
            Tab::Workspaces => Tab::Peers,
            Tab::Peers => Tab::Chat,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Tab::Chat => Tab::Peers,
            Tab::Sessions => Tab::Chat,
            Tab::Launch => Tab::Sessions,
            Tab::Plans => Tab::Launch,
            Tab::System => Tab::Plans,
            Tab::Tasks => Tab::System,
            Tab::Schedules => Tab::Tasks,
            Tab::Scopes => Tab::Schedules,
            Tab::Workspaces => Tab::Scopes,
            Tab::Peers => Tab::Workspaces,
        }
    }
}

// ── plans state ───────────────────────────────────────────────────────────────

pub struct PlansState {
    pub plans: Vec<Plan>,
    pub selected: usize,
    pub table_state: ratatui::widgets::TableState,
}

impl PlansState {
    pub fn new() -> Self {
        Self {
            plans: vec![],
            selected: 0,
            table_state: ratatui::widgets::TableState::default(),
        }
    }

    pub fn selected_plan(&self) -> Option<&Plan> {
        self.plans.get(self.selected)
    }

    pub fn move_up(&mut self) {
        let n = self.plans.len();
        if n == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            n - 1
        } else {
            self.selected - 1
        };
        self.table_state.select(Some(self.selected));
    }

    pub fn move_down(&mut self) {
        let n = self.plans.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
        self.table_state.select(Some(self.selected));
    }
}

// ── scopes state ──────────────────────────────────────────────────────────────

pub struct ScopesState {
    /// Unique scope_keys derived from all known plans.
    pub scopes: Vec<String>,
    pub selected: usize,
}

impl ScopesState {
    pub fn new() -> Self {
        Self {
            scopes: vec![],
            selected: 0,
        }
    }

    pub fn selected_scope(&self) -> Option<&str> {
        self.scopes.get(self.selected).map(String::as_str)
    }

    pub fn move_up(&mut self) {
        let n = self.scopes.len();
        if n == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            n - 1
        } else {
            self.selected - 1
        };
    }

    pub fn move_down(&mut self) {
        let n = self.scopes.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
    }

    pub fn refresh(&mut self, plans: &[Plan]) {
        use std::collections::BTreeSet;
        let mut keys: BTreeSet<String> = BTreeSet::new();
        for p in plans {
            keys.insert(p.scope.scope_key());
        }
        self.scopes = keys.into_iter().collect();
        self.selected = self.selected.min(self.scopes.len().saturating_sub(1));
    }
}

// ── workspaces registry state ─────────────────────────────────────────────────

pub struct WorkspacesRegState {
    pub entries: Vec<orbit_core::workspace_registry::WorkspaceEntry>,
    pub selected: usize,
}

impl WorkspacesRegState {
    pub fn new() -> Self {
        let entries = orbit_core::workspace_registry::WorkspaceRegistry::load().workspaces;
        Self {
            entries,
            selected: 0,
        }
    }

    pub fn refresh(&mut self) {
        let prev_name = self.entries.get(self.selected).map(|e| e.name.clone());
        self.entries = orbit_core::workspace_registry::WorkspaceRegistry::load().workspaces;
        // keep selection on same name if still present
        if let Some(name) = prev_name.and_then(|n| self.entries.iter().position(|e| e.name == n)) {
            self.selected = name;
            return;
        }
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub fn move_up(&mut self) {
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            n - 1
        } else {
            self.selected - 1
        };
    }

    pub fn move_down(&mut self) {
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
    }
}

// ── schedules state ───────────────────────────────────────────────────────────

pub struct SchedulesState {
    pub schedules: Vec<ScheduledPlan>,
    pub selected: usize,
    pub table_state: ratatui::widgets::TableState,
}

impl SchedulesState {
    pub fn new() -> Self {
        Self {
            schedules: vec![],
            selected: 0,
            table_state: ratatui::widgets::TableState::default(),
        }
    }

    pub fn selected_schedule(&self) -> Option<&ScheduledPlan> {
        self.schedules.get(self.selected)
    }

    pub fn move_up(&mut self) {
        let n = self.schedules.len();
        if n == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            n - 1
        } else {
            self.selected - 1
        };
        self.table_state.select(Some(self.selected));
    }

    pub fn move_down(&mut self) {
        let n = self.schedules.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
        self.table_state.select(Some(self.selected));
    }
}

// ── mode (popup overlays) ─────────────────────────────────────────────────────

pub struct WriteJiraState {
    pub key: String,
    pub input: TextInput,
}

pub enum Mode {
    Normal,
    Help,
    ConfirmKill(Session),
    SessionDetails(Session),
    AddMcp(Box<AddMcpState>),
    ConfirmRemoveMcp(crate::mcp::McpEntry),
    FieldSelect(FieldSelectState),
    TaskDetailsLoading,
    TaskDetails(Box<orbit_core::jira::JiraIssueDetail>),
    TaskDetailsError(String),
    AddComment(Box<WriteJiraState>),
    ShareDialog(Box<ShareDialogState>),
}

// ── share / peers state ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShareRole {
    Observer,
    Contributor,
}

pub struct ShareDialogState {
    pub role: ShareRole,
    pub port_input: crate::widget::TextInput,
    pub name_input: crate::widget::TextInput,
    pub status: ShareStatus,
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum ShareStatus {
    Idle,
    Active { port: u16 },
    Error(String),
}

pub struct PeersState {
    pub peers: Vec<orbit_core::net::NetworkPeerInfo>,
    #[allow(dead_code)]
    pub selected: usize,
}

impl PeersState {
    pub fn new() -> Self {
        Self {
            peers: vec![],
            selected: 0,
        }
    }
}

// ── tasks state ───────────────────────────────────────────────────────────────

pub struct TasksState {
    pub items: Vec<orbit_core::task::OrbitTask>,
    pub selected: usize,
    pub table_state: ratatui::widgets::TableState,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    /// Index into sources() + 1; 0 = show all.
    pub source_filter_idx: usize,
    pub last_store_mtime: Option<std::time::SystemTime>,
}

impl TasksState {
    pub fn new() -> Self {
        Self {
            items: vec![],
            selected: 0,
            table_state: ratatui::widgets::TableState::default(),
            loading: false,
            loaded: false,
            error: None,
            source_filter_idx: 0,
            last_store_mtime: None,
        }
    }

    /// Unique source labels across all items (e.g. ["jira", "manual"]).
    pub fn sources(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut sources = Vec::new();
        for item in &self.items {
            let lbl = item.source.label();
            if seen.insert(lbl.clone()) {
                sources.push(lbl);
            }
        }
        sources
    }

    pub fn filtered_count(&self) -> usize {
        if self.source_filter_idx == 0 {
            return self.items.len();
        }
        let sources = self.sources();
        if let Some(src) = sources.get(self.source_filter_idx - 1) {
            self.items
                .iter()
                .filter(|t| &t.source.label() == src)
                .count()
        } else {
            self.items.len()
        }
    }

    pub fn filtered_items(&self) -> Vec<&orbit_core::task::OrbitTask> {
        if self.source_filter_idx == 0 {
            return self.items.iter().collect();
        }
        let sources = self.sources();
        if let Some(src) = sources.get(self.source_filter_idx - 1) {
            self.items
                .iter()
                .filter(|t| &t.source.label() == src)
                .collect()
        } else {
            self.items.iter().collect()
        }
    }

    pub fn selected_task(&self) -> Option<orbit_core::task::OrbitTask> {
        let filtered = self.filtered_items();
        filtered.get(self.selected).map(|t| (*t).clone())
    }

    pub fn move_up(&mut self) {
        let n = self.filtered_count();
        if n == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            n - 1
        } else {
            self.selected - 1
        };
        self.table_state.select(Some(self.selected));
    }

    pub fn move_down(&mut self) {
        let n = self.filtered_count();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
        self.table_state.select(Some(self.selected));
    }

    pub fn cycle_source_right(&mut self) {
        let n = self.sources().len() + 1;
        if n <= 1 {
            return;
        }
        self.source_filter_idx = (self.source_filter_idx + 1) % n;
        self.selected = 0;
        self.table_state.select(Some(0));
    }

    pub fn cycle_source_left(&mut self) {
        let n = self.sources().len() + 1;
        if n <= 1 {
            return;
        }
        self.source_filter_idx = (self.source_filter_idx + n - 1) % n;
        self.selected = 0;
        self.table_state.select(Some(0));
    }
}

// ── engines ───────────────────────────────────────────────────────────────────

pub const ENGINES: [Engine; 3] = [Engine::Opencode, Engine::Gemini, Engine::Claude];

// ── launch state ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LaunchField {
    Engine,
    Workspace,
    Tenant,
    Project,
    Repository,
    Task,
    NoTmux,
    Launch,
}

impl LaunchField {
    pub fn next(self) -> Self {
        match self {
            LaunchField::Engine => LaunchField::Workspace,
            LaunchField::Workspace => LaunchField::Tenant,
            LaunchField::Tenant => LaunchField::Project,
            LaunchField::Project => LaunchField::Repository,
            LaunchField::Repository => LaunchField::Task,
            LaunchField::Task => LaunchField::NoTmux,
            LaunchField::NoTmux => LaunchField::Launch,
            LaunchField::Launch => LaunchField::Engine,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            LaunchField::Engine => LaunchField::Launch,
            LaunchField::Workspace => LaunchField::Engine,
            LaunchField::Tenant => LaunchField::Workspace,
            LaunchField::Project => LaunchField::Tenant,
            LaunchField::Repository => LaunchField::Project,
            LaunchField::Task => LaunchField::Repository,
            LaunchField::NoTmux => LaunchField::Task,
            LaunchField::Launch => LaunchField::NoTmux,
        }
    }

    pub fn is_text_input(self) -> bool {
        matches!(
            self,
            LaunchField::Tenant | LaunchField::Project | LaunchField::Repository
        )
    }
}

pub struct LaunchState {
    pub engine_idx: usize,
    pub workspace_idx: usize,
    pub workspaces: Vec<String>,
    pub tenant: TextInput,
    pub project: TextInput,
    pub repository: TextInput,
    pub no_tmux: bool,
    pub focused: LaunchField,
    /// Task selected from the Jira Tasks tab; pre-fills context at launch.
    pub task_context: Option<orbit_core::jira::TaskContext>,
}

impl LaunchState {
    pub fn new(default_tenant: &str, default_engine: &str) -> Self {
        let engine_idx = ENGINES
            .iter()
            .position(|e| e.as_str() == default_engine)
            .unwrap_or(0);

        let workspaces = detect_workspace_names();

        Self {
            engine_idx,
            workspace_idx: 0,
            workspaces,
            tenant: TextInput::new("Tenant", "AIDEV").with_value(default_tenant),
            project: TextInput::new("Project", "my-project"),
            repository: TextInput::new("Repo", "(optional)"),
            no_tmux: false,
            focused: LaunchField::Engine,
            task_context: None,
        }
    }

    pub fn engine(&self) -> Engine {
        ENGINES[self.engine_idx]
    }

    pub fn workspace_name(&self) -> &str {
        self.workspaces
            .get(self.workspace_idx)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn cycle_workspace_left(&mut self) {
        let n = self.workspaces.len();
        if n == 0 {
            return;
        }
        self.workspace_idx = (self.workspace_idx + n - 1) % n;
        self.reset_scope_fields();
    }

    pub fn cycle_workspace_right(&mut self) {
        let n = self.workspaces.len();
        if n == 0 {
            return;
        }
        self.workspace_idx = (self.workspace_idx + 1) % n;
        self.reset_scope_fields();
    }

    fn reset_scope_fields(&mut self) {
        self.tenant = TextInput::new("Tenant", "AIDEV");
        self.project = TextInput::new("Project", "my-project");
        self.repository = TextInput::new("Repo", "(optional)");
    }

    pub fn to_params(&self) -> LaunchParams {
        LaunchParams {
            engine: self.engine(),
            workspace: self.workspace_name().to_string(),
            tenant: self.tenant.as_str().to_string(),
            project: self.project.as_str().to_string(),
            repository: self.repository.as_str().to_string(),
            no_tmux: self.no_tmux,
            task_context: self.task_context.clone(),
        }
    }

    pub fn focused_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.focused {
            LaunchField::Tenant => Some(&mut self.tenant),
            LaunchField::Project => Some(&mut self.project),
            LaunchField::Repository => Some(&mut self.repository),
            _ => None,
        }
    }
}

fn detect_workspace_names() -> Vec<String> {
    let home = dirs_home();
    let Ok(rd) = std::fs::read_dir(&home) else {
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

// ── field select state ────────────────────────────────────────────────────────

pub struct FieldSelectState {
    pub field: LaunchField,
    pub options: Vec<String>,
    pub cursor: usize,
    pub filter: String,
}

impl FieldSelectState {
    pub fn filtered_options(&self) -> Vec<&str> {
        if self.filter.is_empty() {
            self.options.iter().map(|s| s.as_str()).collect()
        } else {
            let f = self.filter.to_lowercase();
            self.options
                .iter()
                .filter(|o| o.to_lowercase().contains(&f))
                .map(|s| s.as_str())
                .collect()
        }
    }

    pub fn selected_option(&self) -> Option<&str> {
        self.filtered_options().into_iter().nth(self.cursor)
    }

    pub fn move_up(&mut self) {
        let n = self.filtered_options().len();
        if n == 0 {
            return;
        }
        self.cursor = if self.cursor == 0 {
            n - 1
        } else {
            self.cursor - 1
        };
    }

    pub fn move_down(&mut self) {
        let n = self.filtered_options().len();
        if n == 0 {
            return;
        }
        self.cursor = (self.cursor + 1) % n;
    }
}

// ── mcp scope ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum McpScope {
    Global,
    Tenant,
    Project,
    Repo,
}

impl McpScope {
    pub fn cycle_right(self) -> Self {
        match self {
            McpScope::Global => McpScope::Tenant,
            McpScope::Tenant => McpScope::Project,
            McpScope::Project => McpScope::Repo,
            McpScope::Repo => McpScope::Global,
        }
    }

    pub fn cycle_left(self) -> Self {
        match self {
            McpScope::Global => McpScope::Repo,
            McpScope::Tenant => McpScope::Global,
            McpScope::Project => McpScope::Tenant,
            McpScope::Repo => McpScope::Project,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            McpScope::Global => "global",
            McpScope::Tenant => "tenant",
            McpScope::Project => "project",
            McpScope::Repo => "repo",
        }
    }

    pub fn needs_project(self) -> bool {
        matches!(self, McpScope::Project | McpScope::Repo)
    }

    pub fn needs_repo(self) -> bool {
        self == McpScope::Repo
    }
}

// ── add-mcp state ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddMcpField {
    Name,
    Command,
    Args,
    Env,
    Scope,
    ProjectName,
    RepoName,
    Confirm,
}

impl AddMcpField {
    pub fn next(self) -> Self {
        match self {
            AddMcpField::Name => AddMcpField::Command,
            AddMcpField::Command => AddMcpField::Args,
            AddMcpField::Args => AddMcpField::Env,
            AddMcpField::Env => AddMcpField::Scope,
            AddMcpField::Scope => AddMcpField::ProjectName,
            AddMcpField::ProjectName => AddMcpField::RepoName,
            AddMcpField::RepoName => AddMcpField::Confirm,
            AddMcpField::Confirm => AddMcpField::Name,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            AddMcpField::Name => AddMcpField::Confirm,
            AddMcpField::Command => AddMcpField::Name,
            AddMcpField::Args => AddMcpField::Command,
            AddMcpField::Env => AddMcpField::Args,
            AddMcpField::Scope => AddMcpField::Env,
            AddMcpField::ProjectName => AddMcpField::Scope,
            AddMcpField::RepoName => AddMcpField::ProjectName,
            AddMcpField::Confirm => AddMcpField::RepoName,
        }
    }
}

pub struct AddMcpState {
    pub name: TextInput,
    pub command: TextInput,
    pub args: TextInput,
    pub env: TextInput,
    pub scope: McpScope,
    pub project_name: TextInput,
    pub repo_name: TextInput,
    pub focused: AddMcpField,
}

impl AddMcpState {
    pub fn new() -> Self {
        Self {
            name: TextInput::new("Name", "server-name"),
            command: TextInput::new("Command", "npx"),
            args: TextInput::new("Args", "-y @scope/mcp-package"),
            env: TextInput::new("Env", "KEY=VALUE"),
            scope: McpScope::Global,
            project_name: TextInput::new("Project", "my-project"),
            repo_name: TextInput::new("Repo", "my-repo"),
            focused: AddMcpField::Name,
        }
    }

    pub fn focused_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.focused {
            AddMcpField::Name => Some(&mut self.name),
            AddMcpField::Command => Some(&mut self.command),
            AddMcpField::Args => Some(&mut self.args),
            AddMcpField::Env => Some(&mut self.env),
            AddMcpField::ProjectName => Some(&mut self.project_name),
            AddMcpField::RepoName => Some(&mut self.repo_name),
            _ => None,
        }
    }

    pub fn target_path(&self, ai_root: &Path, default_tenant: &str) -> PathBuf {
        let tenant = if default_tenant.is_empty() {
            "default"
        } else {
            default_tenant
        };
        match self.scope {
            McpScope::Global => ai_root.join("mcp.json"),
            McpScope::Tenant => ai_root.join("tenants").join(tenant).join("mcp.json"),
            McpScope::Project => ai_root
                .join("tenants")
                .join(tenant)
                .join("projects")
                .join(self.project_name.as_str())
                .join("mcp.json"),
            McpScope::Repo => ai_root
                .join("tenants")
                .join(tenant)
                .join("projects")
                .join(self.project_name.as_str())
                .join("repositories")
                .join(self.repo_name.as_str())
                .join("mcp.json"),
        }
    }

    pub fn args_list(&self) -> Vec<String> {
        self.args
            .as_str()
            .split_whitespace()
            .map(String::from)
            .collect()
    }

    pub fn env_map(&self) -> std::collections::HashMap<String, String> {
        self.env
            .as_str()
            .split(',')
            .filter_map(|pair| pair.trim().split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.to_string()))
            .collect()
    }
}

// ── system state ──────────────────────────────────────────────────────────────

pub struct SystemState {
    pub ai_root: PathBuf,
    pub default_engine: String,
    pub default_tenant: String,
    pub install_dir: PathBuf,
    pub dev_mode: bool,
    pub daemon_running: bool,
    pub mcp_entries: Vec<crate::mcp::McpEntry>,
    pub mcp_selected: usize,
}

impl SystemState {
    pub fn load() -> Self {
        let cfg = UserConfig::load();
        let ai_root = cfg.ai_root_expanded();
        let install_dir = cfg.install_dir_expanded();
        let dev_mode = is_dev_mode(&install_dir);
        let daemon_running = socket_path().exists();
        let mcp_entries = crate::mcp::load_entries(&ai_root, &cfg.engine.default_tenant);

        Self {
            ai_root,
            default_engine: cfg.engine.default.clone(),
            default_tenant: cfg.engine.default_tenant.clone(),
            install_dir,
            dev_mode,
            daemon_running,
            mcp_entries,
            mcp_selected: 0,
        }
    }

    pub fn refresh(&mut self) {
        self.dev_mode = is_dev_mode(&self.install_dir);
        self.daemon_running = socket_path().exists();
        self.reload_mcp();
    }

    pub fn reload_mcp(&mut self) {
        self.mcp_entries = crate::mcp::load_entries(&self.ai_root, &self.default_tenant);
        self.mcp_selected = self
            .mcp_selected
            .min(self.mcp_entries.len().saturating_sub(1));
    }
}

fn is_dev_mode(install_dir: &Path) -> bool {
    let orbit = install_dir.join("orbit");
    let orbit_dev = install_dir.join("orbit-dev");

    if !orbit
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return false;
    }

    orbit
        .read_link()
        .map(|target| {
            let abs = if target.is_absolute() {
                target
            } else {
                orbit.parent().unwrap_or(Path::new(".")).join(target)
            };
            abs == orbit_dev
        })
        .unwrap_or(false)
}

// ── async actions ─────────────────────────────────────────────────────────────

pub enum AsyncAction {
    ChatSubmit(String),
    ChatApprove { plan_id: String, node_id: String },
    ChatCancel(Option<String>),
    ChatListPlans,
    DaemonStart,
    DaemonStop,
    RefreshPlans,
    CancelPlan(String),
    ApprovePlanNode {
        plan_id: String,
        node_id: String,
    },
    RefreshSchedules,
    CancelSchedule(String),
    RunScheduleNow(String),
    /// Load tasks from cache; falls back to direct acli call if no cache exists.
    RefreshTasks,
    /// Force a direct acli call, bypassing cache (triggered by [r]).
    ForceRefreshTasks,
    FetchTaskDetail(String),
    AddComment {
        key: String,
        body: String,
    },
    StartSharing {
        role: ShareRole,
        port: u16,
        name: String,
    },
    StopSharing,
    RefreshPeers,
}

// ── post-exit actions ─────────────────────────────────────────────────────────

enum PostAction {
    Attach(Session),
    Launch(LaunchParams),
}

// ── app ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub tab: Tab,
    pub mode: Mode,
    pub chat: chat::ChatState,
    pub sessions: Vec<Session>,
    pub table_state: TableState,
    pub launch: LaunchState,
    pub sys: SystemState,
    pub plans: PlansState,
    pub scopes: ScopesState,
    pub schedules: SchedulesState,
    pub workspaces_reg: WorkspacesRegState,
    pub tasks: TasksState,
    pub jira_enabled: bool,
    pub status_msg: Option<String>,
    pub pending_async: Option<AsyncAction>,
    pub workspaces: Vec<PathBuf>,
    pub workspace_idx: usize,
    pub palette: crate::theme::Palette,
    pub serving_active: bool,
    pub peers_state: PeersState,
    should_quit: bool,
    post_action: Option<PostAction>,
}

impl App {
    fn new() -> Self {
        let sys = SystemState::load();
        let launch = LaunchState::new(&sys.default_tenant, &sys.default_engine);
        let jira_enabled = orbit_core::plugin::load_all()
            .iter()
            .find(|p| p.name == "jira")
            .map(|p| p.is_installed())
            .unwrap_or(false);

        let sessions = Session::load_all();
        let mut table_state = TableState::default();
        if !sessions.is_empty() {
            table_state.select(Some(0));
        }

        let home = dirs_home();
        let workspaces = detect_workspaces(&home);
        let workspace_idx = workspaces
            .iter()
            .position(|w| *w == sys.ai_root)
            .unwrap_or(0);

        Self {
            tab: Tab::Chat,
            mode: Mode::Normal,
            chat: chat::ChatState::new(),
            sessions,
            table_state,
            launch,
            sys,
            plans: PlansState::new(),
            scopes: ScopesState::new(),
            schedules: SchedulesState::new(),
            workspaces_reg: WorkspacesRegState::new(),
            tasks: TasksState::new(),
            jira_enabled,
            status_msg: None,
            pending_async: None,
            workspaces,
            workspace_idx,
            palette: crate::theme::Palette::detect(),
            serving_active: false,
            peers_state: PeersState::new(),
            should_quit: false,
            post_action: None,
        }
    }

    pub fn active_workspace_name(&self) -> &str {
        self.workspaces
            .get(self.workspace_idx)
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("?")
    }

    pub fn switch_workspace_next(&mut self) {
        if self.workspaces.len() <= 1 {
            return;
        }
        self.workspace_idx = (self.workspace_idx + 1) % self.workspaces.len();
        let new_root = self.workspaces[self.workspace_idx].clone();
        self.sys.ai_root = new_root;
        self.sys.reload_mcp();
        self.launch = LaunchState::new(&self.sys.default_tenant, &self.sys.default_engine);
        self.refresh_sessions();
        self.status_msg = Some(format!("Workspace: {}", self.active_workspace_name()));
    }

    pub fn refresh_sessions(&mut self) {
        let selected = self.table_state.selected();
        self.sessions = Session::load_all();
        if self.sessions.is_empty() {
            self.table_state.select(None);
        } else {
            let i = selected.unwrap_or(0).min(self.sessions.len() - 1);
            self.table_state.select(Some(i));
        }
    }

    pub fn move_up(&mut self) {
        let n = self.sessions.len();
        if n == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state
            .select(Some(if i == 0 { n - 1 } else { i - 1 }));
    }

    pub fn move_down(&mut self) {
        let n = self.sessions.len();
        if n == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1) % n));
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.table_state
            .selected()
            .and_then(|i| self.sessions.get(i))
    }

    pub fn attach_selected(&mut self) {
        match self.selected_session() {
            None => {
                self.status_msg = Some("No session selected.".to_string());
            }
            Some(s) if !s.has_tmux() => {
                self.status_msg = Some(format!("Session {} was not launched in tmux.", s.id));
            }
            Some(s) if !s.is_running() => {
                self.status_msg = Some(format!(
                    "Session {} is no longer running. Run `orbit session clean`.",
                    s.id
                ));
            }
            Some(s) if !s.tmux_window_exists() => {
                self.status_msg = Some(format!(
                    "tmux window for session {} is gone. Run `orbit session clean`.",
                    s.id
                ));
            }
            Some(s) => {
                self.post_action = Some(PostAction::Attach(s.clone()));
                self.should_quit = true;
            }
        }
    }

    pub fn clean_dead(&mut self) {
        let sessions = std::mem::take(&mut self.sessions);
        let mut cleaned = 0usize;
        for s in &sessions {
            if !s.is_running() {
                let _ = s.delete();
                cleaned += 1;
            }
        }
        self.status_msg = Some(if cleaned > 0 {
            format!("Cleaned {cleaned} dead session(s).")
        } else {
            "No dead sessions to clean.".to_string()
        });
        self.refresh_sessions();
    }

    pub fn mcp_move_up(&mut self) {
        let n = self.sys.mcp_entries.len();
        if n == 0 {
            return;
        }
        self.sys.mcp_selected = if self.sys.mcp_selected == 0 {
            n - 1
        } else {
            self.sys.mcp_selected - 1
        };
    }

    pub fn mcp_move_down(&mut self) {
        let n = self.sys.mcp_entries.len();
        if n == 0 {
            return;
        }
        self.sys.mcp_selected = (self.sys.mcp_selected + 1) % n;
    }

    pub fn selected_mcp(&self) -> Option<&crate::mcp::McpEntry> {
        self.sys.mcp_entries.get(self.sys.mcp_selected)
    }

    pub fn handle_key(&mut self, code: KeyCode, _mods: KeyModifiers) {
        if !matches!(self.mode, Mode::Normal) {
            self.handle_popup_key(code);
            return;
        }

        // Chat tab: route all keys to its own handler (manages its own input mode).
        if self.tab == Tab::Chat {
            self.handle_chat_key(code);
            return;
        }

        // ? always opens help (except when typing in Launch tab)
        if code == KeyCode::Char('?') && !self.launch.focused.is_text_input() {
            self.mode = Mode::Help;
            return;
        }

        // Tab switching + workspace cycling: skip when typing in a text field
        let in_text_input = self.tab == Tab::Launch && self.launch.focused.is_text_input();
        if !in_text_input {
            match code {
                KeyCode::Tab => {
                    self.tab = self.tab.next();
                    if self.tab == Tab::Tasks && !self.tasks.loaded && !self.tasks.loading {
                        self.pending_async = Some(AsyncAction::RefreshTasks);
                    }
                    return;
                }
                KeyCode::BackTab => {
                    self.tab = self.tab.prev();
                    if self.tab == Tab::Tasks && !self.tasks.loaded && !self.tasks.loading {
                        self.pending_async = Some(AsyncAction::RefreshTasks);
                    }
                    return;
                }
                KeyCode::Char('1') => {
                    self.tab = Tab::Sessions;
                    return;
                }
                KeyCode::Char('2') => {
                    self.tab = Tab::Launch;
                    return;
                }
                KeyCode::Char('3') => {
                    self.tab = Tab::Plans;
                    self.pending_async = Some(AsyncAction::RefreshPlans);
                    return;
                }
                KeyCode::Char('4') => {
                    self.tab = Tab::System;
                    return;
                }
                KeyCode::Char('5') => {
                    self.tab = Tab::Tasks;
                    if !self.tasks.loaded && !self.tasks.loading {
                        self.pending_async = Some(AsyncAction::RefreshTasks);
                    }
                    return;
                }
                KeyCode::Char('6') => {
                    self.tab = Tab::Schedules;
                    self.pending_async = Some(AsyncAction::RefreshSchedules);
                    return;
                }
                KeyCode::Char('7') => {
                    self.tab = Tab::Scopes;
                    self.pending_async = Some(AsyncAction::RefreshPlans);
                    return;
                }
                KeyCode::Char('8') => {
                    self.tab = Tab::Workspaces;
                    self.workspaces_reg.refresh();
                    self.pending_async = Some(AsyncAction::RefreshPlans);
                    return;
                }
                KeyCode::Char('9') => {
                    self.tab = Tab::Peers;
                    self.pending_async = Some(AsyncAction::RefreshPeers);
                    return;
                }
                KeyCode::Char('w') => {
                    self.switch_workspace_next();
                    return;
                }
                _ => {}
            }
        }

        match self.tab {
            Tab::Chat => {} // handled above before tab-switching
            Tab::Sessions => self.handle_sessions_key(code),
            Tab::Launch => self.handle_launch_key(code),
            Tab::Plans => self.handle_plans_key(code),
            Tab::System => self.handle_system_key(code),
            Tab::Tasks => self.handle_tasks_key(code),
            Tab::Schedules => self.handle_schedules_key(code),
            Tab::Scopes => self.handle_scopes_key(code),
            Tab::Workspaces => self.handle_workspaces_key(code),
            Tab::Peers => self.handle_peers_key(code),
        }
    }

    fn handle_chat_key(&mut self, code: KeyCode) {
        use chat::ChatMode;

        // Tab and numbered keys always work for navigation.
        match code {
            KeyCode::Tab => {
                self.tab = self.tab.next();
                return;
            }
            KeyCode::BackTab => {
                self.tab = self.tab.prev();
                return;
            }
            KeyCode::Char('0') => {
                self.tab = Tab::Chat;
                return;
            }
            KeyCode::Char('1') => { self.tab = Tab::Sessions; return; }
            KeyCode::Char('2') => { self.tab = Tab::Launch; return; }
            KeyCode::Char('3') => { self.tab = Tab::Plans; self.pending_async = Some(AsyncAction::RefreshPlans); return; }
            KeyCode::Char('4') => { self.tab = Tab::System; return; }
            KeyCode::Char('5') => {
                self.tab = Tab::Tasks;
                if !self.tasks.loaded && !self.tasks.loading {
                    self.pending_async = Some(AsyncAction::RefreshTasks);
                }
                return;
            }
            KeyCode::Char('6') => { self.tab = Tab::Schedules; self.pending_async = Some(AsyncAction::RefreshSchedules); return; }
            KeyCode::Char('q') if self.chat.mode == ChatMode::Scroll => {
                self.should_quit = true;
                return;
            }
            _ => {}
        }

        match self.chat.mode {
            ChatMode::Input => match code {
                KeyCode::Esc => {
                    self.chat.mode = ChatMode::Scroll;
                }
                KeyCode::Enter => {
                    if let Some(text) = chat::take_input(&mut self.chat) {
                        self.pending_async = Some(AsyncAction::ChatSubmit(text));
                    }
                }
                other => {
                    self.chat.input.handle_key(other);
                }
            },
            ChatMode::Scroll => match code {
                KeyCode::Char('i') | KeyCode::Char('a') => {
                    self.chat.mode = ChatMode::Input;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.chat.scroll_up();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.chat.scroll_down(u16::MAX);
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
        }
    }

    fn handle_popup_key(&mut self, code: KeyCode) {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Help => {
                // any key closes help; mode already set to Normal
            }
            Mode::ConfirmKill(session) => {
                if matches!(code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                    let id = session.id.clone();
                    send_sigterm(session.pid);
                    let _ = session.delete();
                    self.status_msg = Some(format!("Killed session {id}"));
                    self.refresh_sessions();
                }
                // any other key: cancel (mode already Normal)
            }
            Mode::SessionDetails(session) => match code {
                KeyCode::Char('a') => {
                    if !session.has_tmux() {
                        self.status_msg =
                            Some("Session was not launched in tmux — cannot attach.".into());
                    } else if !session.is_running() {
                        self.status_msg =
                            Some("Session is no longer running. Run `orbit session clean`.".into());
                    } else if !session.tmux_window_exists() {
                        self.status_msg =
                            Some("tmux window is gone. Run `orbit session clean`.".into());
                    } else {
                        self.post_action = Some(PostAction::Attach(session));
                        self.should_quit = true;
                    }
                }
                KeyCode::Char('K') => {
                    let id = session.id.clone();
                    send_sigterm(session.pid);
                    let _ = session.delete();
                    self.status_msg = Some(format!("Killed session {id}"));
                    self.refresh_sessions();
                }
                _ => {
                    // Esc or any other key: close popup
                }
            },
            Mode::AddMcp(mut state) => match code {
                KeyCode::Esc => {}
                KeyCode::Enter if state.focused == AddMcpField::Confirm => {
                    let name = state.name.as_str().to_string();
                    let cmd = state.command.as_str().to_string();
                    if name.is_empty() || cmd.is_empty() {
                        self.status_msg = Some("Name and Command are required.".into());
                        self.mode = Mode::AddMcp(state);
                    } else {
                        let path = state.target_path(&self.sys.ai_root, &self.sys.default_tenant);
                        let args = state.args_list();
                        let env_map = state.env_map();
                        match crate::mcp::add_server(&path, &name, &cmd, &args, env_map) {
                            Ok(()) => {
                                self.status_msg = Some(format!("Added MCP server \"{name}\"."));
                                self.sys.reload_mcp();
                            }
                            Err(e) => {
                                self.status_msg = Some(format!("Failed to add MCP server: {e}"));
                            }
                        }
                        // mode stays Normal (success or I/O error)
                    }
                }
                _ => {
                    let consumed = if let Some(input) = state.focused_input_mut() {
                        input.handle_key(code)
                    } else {
                        false
                    };
                    if !consumed {
                        match code {
                            KeyCode::Up => state.focused = state.focused.prev(),
                            KeyCode::Down | KeyCode::Enter => state.focused = state.focused.next(),
                            KeyCode::Right if state.focused == AddMcpField::Scope => {
                                state.scope = state.scope.cycle_right();
                            }
                            KeyCode::Left if state.focused == AddMcpField::Scope => {
                                state.scope = state.scope.cycle_left();
                            }
                            _ => {}
                        }
                    }
                    self.mode = Mode::AddMcp(state); // state is Box<AddMcpState>
                }
            },
            Mode::ConfirmRemoveMcp(entry) => {
                if matches!(code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                    let name = entry.name.clone();
                    let path = entry.source_file.clone();
                    match crate::mcp::remove_server(&path, &name) {
                        Ok(()) => {
                            self.status_msg = Some(format!("Removed \"{name}\"."));
                            self.sys.reload_mcp();
                        }
                        Err(e) => {
                            self.status_msg = Some(format!("Failed to remove MCP: {e}"));
                        }
                    }
                }
                // any other key: cancel (mode stays Normal)
            }
            Mode::FieldSelect(mut state) => {
                match code {
                    KeyCode::Esc => { /* mode already reset to Normal */ }
                    KeyCode::Up => {
                        state.move_up();
                        self.mode = Mode::FieldSelect(state);
                    }
                    KeyCode::Down => {
                        state.move_down();
                        self.mode = Mode::FieldSelect(state);
                    }
                    KeyCode::Enter => {
                        if let Some(selected) = state.selected_option().map(|s| s.to_string()) {
                            match state.field {
                                LaunchField::Tenant => {
                                    self.launch.tenant =
                                        TextInput::new("Tenant", "AIDEV").with_value(&selected);
                                }
                                LaunchField::Project => {
                                    self.launch.project = TextInput::new("Project", "my-project")
                                        .with_value(&selected);
                                }
                                LaunchField::Repository => {
                                    self.launch.repository =
                                        TextInput::new("Repo", "(optional)").with_value(&selected);
                                }
                                _ => {}
                            }
                        }
                        // mode reset to Normal by the std::mem::replace above
                    }
                    KeyCode::Backspace => {
                        state.filter.pop();
                        state.cursor = 0;
                        self.mode = Mode::FieldSelect(state);
                    }
                    KeyCode::Char(c) => {
                        state.filter.push(c);
                        state.cursor = 0;
                        self.mode = Mode::FieldSelect(state);
                    }
                    _ => {
                        self.mode = Mode::FieldSelect(state);
                    }
                }
            }
            Mode::TaskDetailsLoading | Mode::TaskDetailsError(_) => {
                // any key closes the popup
            }
            Mode::TaskDetails(detail) => match code {
                KeyCode::Char('c') => {
                    self.mode = Mode::AddComment(Box::new(WriteJiraState {
                        key: detail.key.clone(),
                        input: TextInput::new("comment", "Write a comment…"),
                    }));
                }
                KeyCode::Char('e') => {
                    // Open in browser to preserve rich content (tables, images, etc.)
                    let key = detail.key.clone();
                    let _ = std::process::Command::new("acli")
                        .args(["jira", "workitem", "view", &key, "--web"])
                        .spawn();
                }
                _ => {} // any other key closes
            },
            Mode::AddComment(mut state) => match code {
                KeyCode::Esc => {} // closes, mode already Normal
                KeyCode::Enter => {
                    let body = state.input.value.trim().to_string();
                    if !body.is_empty() {
                        self.pending_async = Some(AsyncAction::AddComment {
                            key: state.key.clone(),
                            body,
                        });
                    }
                }
                other => {
                    state.input.handle_key(other);
                    self.mode = Mode::AddComment(state);
                }
            },
            Mode::ShareDialog(mut state) => match code {
                KeyCode::Esc => {
                    // close, mode already Normal
                }
                KeyCode::Enter => {
                    let port: u16 = state.port_input.as_str().parse().unwrap_or(7373);
                    let name = state.name_input.as_str().to_string();
                    let role = state.role;
                    if matches!(state.status, ShareStatus::Active { .. }) {
                        self.pending_async = Some(AsyncAction::StopSharing);
                        state.status = ShareStatus::Idle;
                    } else {
                        self.pending_async = Some(AsyncAction::StartSharing { role, port, name });
                    }
                    self.mode = Mode::ShareDialog(state);
                }
                KeyCode::Left | KeyCode::Right => {
                    state.role = match state.role {
                        ShareRole::Observer => ShareRole::Contributor,
                        ShareRole::Contributor => ShareRole::Observer,
                    };
                    self.mode = Mode::ShareDialog(state);
                }
                _ => {
                    self.mode = Mode::ShareDialog(state);
                }
            },
            Mode::Normal => {}
        }
    }

    fn handle_sessions_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('a') | KeyCode::Enter => self.attach_selected(),
            KeyCode::Char('K') => {
                if let Some(session) = self.selected_session().cloned() {
                    self.mode = Mode::ConfirmKill(session);
                }
            }
            KeyCode::Char('d') => {
                if let Some(session) = self.selected_session().cloned() {
                    self.mode = Mode::SessionDetails(session);
                }
            }
            KeyCode::Char('c') => self.clean_dead(),
            KeyCode::Char('r') => self.refresh_sessions(),
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_launch_key(&mut self, code: KeyCode) {
        // Special handling for the read-only Task field
        if self.launch.focused == LaunchField::Task {
            match code {
                KeyCode::Esc if self.launch.task_context.is_some() => {
                    self.launch.task_context = None;
                    return;
                }
                KeyCode::Char('t') => {
                    self.tab = Tab::Tasks;
                    if !self.tasks.loaded && !self.tasks.loading {
                        self.pending_async = Some(AsyncAction::RefreshTasks);
                    }
                    return;
                }
                _ => {}
            }
        }

        // ↓ on a text field opens the field selector if options are available
        if code == KeyCode::Down && self.launch.focused.is_text_input() {
            let options = load_field_options(self.launch.focused, &self.launch, &self.sys.ai_root);
            if !options.is_empty() {
                let filter = self
                    .launch
                    .focused_input_mut()
                    .map(|i| i.as_str().to_string())
                    .unwrap_or_default();
                self.mode = Mode::FieldSelect(FieldSelectState {
                    field: self.launch.focused,
                    options,
                    cursor: 0,
                    filter,
                });
                return;
            }
            // No options — fall through to next-field navigation
        }

        // Route to focused text input first
        let consumed = {
            if let Some(input) = self.launch.focused_input_mut() {
                input.handle_key(code)
            } else {
                false
            }
        };
        if consumed {
            return;
        }

        match code {
            KeyCode::Up => self.launch.focused = self.launch.focused.prev(),
            KeyCode::Down => self.launch.focused = self.launch.focused.next(),
            KeyCode::Left => {
                if self.launch.focused == LaunchField::Workspace {
                    self.launch.cycle_workspace_left();
                } else {
                    let n = ENGINES.len();
                    self.launch.engine_idx = (self.launch.engine_idx + n - 1) % n;
                }
            }
            KeyCode::Right => {
                if self.launch.focused == LaunchField::Workspace {
                    self.launch.cycle_workspace_right();
                } else {
                    self.launch.engine_idx = (self.launch.engine_idx + 1) % ENGINES.len();
                }
            }
            KeyCode::Char(' ') => {
                if self.launch.focused == LaunchField::NoTmux {
                    self.launch.no_tmux = !self.launch.no_tmux;
                }
            }
            KeyCode::Enter => {
                if self.launch.focused == LaunchField::Launch {
                    let params = self.launch.to_params();
                    self.post_action = Some(PostAction::Launch(params));
                    self.should_quit = true;
                } else {
                    self.launch.focused = self.launch.focused.next();
                }
            }
            KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }

    fn handle_plans_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.plans.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.plans.move_down(),
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(plan) = self.plans.selected_plan() {
                    let id = plan.id.clone();
                    self.pending_async = Some(AsyncAction::CancelPlan(id));
                }
            }
            KeyCode::Char('a') => {
                // Approve the first AwaitingApproval node in the selected plan.
                if let Some(plan) = self.plans.selected_plan() {
                    let waiting = plan
                        .nodes
                        .iter()
                        .find(|n| n.status == orbit_core::plan::NodeStatus::AwaitingApproval);
                    if let Some(node) = waiting {
                        self.pending_async = Some(AsyncAction::ApprovePlanNode {
                            plan_id: plan.id.clone(),
                            node_id: node.id.clone(),
                        });
                    } else {
                        self.status_msg = Some("No node awaiting approval.".into());
                    }
                }
            }
            KeyCode::Char('r') => {
                self.pending_async = Some(AsyncAction::RefreshPlans);
            }
            KeyCode::Char('q') | KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }

    fn handle_schedules_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.schedules.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.schedules.move_down(),
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(sched) = self.schedules.selected_schedule() {
                    let id = sched.id.clone();
                    self.pending_async = Some(AsyncAction::CancelSchedule(id));
                }
            }
            KeyCode::Char('R') => {
                if let Some(sched) = self.schedules.selected_schedule() {
                    let id = sched.id.clone();
                    self.pending_async = Some(AsyncAction::RunScheduleNow(id));
                }
            }
            KeyCode::Char('r') => {
                self.pending_async = Some(AsyncAction::RefreshSchedules);
            }
            KeyCode::Char('q') | KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }

    fn handle_scopes_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.scopes.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.scopes.move_down(),
            KeyCode::Char('r') => {
                self.pending_async = Some(AsyncAction::RefreshPlans);
            }
            KeyCode::Char('q') | KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }

    fn handle_workspaces_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.workspaces_reg.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.workspaces_reg.move_down(),
            KeyCode::Char('r') => {
                self.workspaces_reg.refresh();
                self.pending_async = Some(AsyncAction::RefreshPlans);
            }
            KeyCode::Char('q') | KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }

    fn handle_peers_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('r') => {
                self.pending_async = Some(AsyncAction::RefreshPeers);
            }
            KeyCode::Char('S') => {
                let hostname = hostname_or_orbit();
                self.mode = Mode::ShareDialog(Box::new(ShareDialogState {
                    role: ShareRole::Observer,
                    port_input: crate::widget::TextInput::new("Port", "7373").with_value("7373"),
                    name_input: crate::widget::TextInput::new("Name", "orbit")
                        .with_value(&hostname),
                    status: ShareStatus::Idle,
                }));
            }
            KeyCode::Char('q') | KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }

    fn handle_system_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.mcp_move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.mcp_move_down(),
            KeyCode::Char('a') => {
                self.mode = Mode::AddMcp(Box::new(AddMcpState::new()));
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(entry) = self.selected_mcp().cloned() {
                    self.mode = Mode::ConfirmRemoveMcp(entry);
                }
            }
            KeyCode::Char('s') => {
                self.pending_async = Some(if self.sys.daemon_running {
                    AsyncAction::DaemonStop
                } else {
                    AsyncAction::DaemonStart
                });
            }
            KeyCode::Char('r') => self.sys.refresh(),
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_tasks_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.tasks.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.tasks.move_down(),
            KeyCode::Left => self.tasks.cycle_source_left(),
            KeyCode::Right => self.tasks.cycle_source_right(),
            KeyCode::Enter => {
                if let Some(task) = self.tasks.selected_task() {
                    self.launch.task_context = Some(orbit_core::jira::TaskContext::from(task));
                    self.tab = Tab::Launch;
                    self.launch.focused = LaunchField::Task;
                }
            }
            KeyCode::Char('d') => {
                // Only available for Jira-sourced tasks.
                if let Some(task) = self.tasks.selected_task()
                    && let orbit_core::task::TaskSource::Plugin {
                        name, external_id, ..
                    } = &task.source
                    && name == "jira"
                {
                    self.mode = Mode::TaskDetailsLoading;
                    self.pending_async = Some(AsyncAction::FetchTaskDetail(external_id.clone()));
                }
            }
            KeyCode::Char('e') => {
                if let Some(task) = self.tasks.selected_task()
                    && let orbit_core::task::TaskSource::Plugin { url: Some(url), .. } =
                        &task.source
                {
                    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                }
            }
            KeyCode::Char('r') => {
                self.pending_async = Some(AsyncAction::ForceRefreshTasks);
            }
            KeyCode::Char('q') | KeyCode::Esc => self.tab = Tab::Sessions,
            _ => {}
        }
    }
}

// ── field options loader ──────────────────────────────────────────────────────

pub fn load_field_options(field: LaunchField, launch: &LaunchState, ai_root: &Path) -> Vec<String> {
    fn subdirs(dir: &Path) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(dir) else {
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

    match field {
        LaunchField::Tenant => subdirs(&ai_root.join("tenants")),
        LaunchField::Project => {
            let tenant = launch.tenant.as_str();
            if tenant.is_empty() {
                return vec![];
            }
            subdirs(&ai_root.join("tenants").join(tenant).join("projects"))
        }
        LaunchField::Repository => {
            let tenant = launch.tenant.as_str();
            let project = launch.project.as_str();
            if tenant.is_empty() || project.is_empty() {
                return vec![];
            }
            subdirs(
                &ai_root
                    .join("tenants")
                    .join(tenant)
                    .join("projects")
                    .join(project)
                    .join("repositories"),
            )
        }
        _ => vec![],
    }
}

// ── event loop ────────────────────────────────────────────────────────────────

async fn handle_async_action(action: AsyncAction, app: &mut App) {
    match action {
        AsyncAction::ChatSubmit(text) => {
            use orbit_core::ipc::{Request, Response};

            let action = chat::handle_submit(&mut app.chat, text);
            let Some(a) = action else { return; };

            match a {
                chat::ChatAsyncAction::CreatePlan { goal, dry_run } => {
                    let req = Request::CreatePlan {
                        intent: goal,
                        workspace: None,
                        tenant: None,
                        project: None,
                        repository: None,
                        dry_run,
                        verbose: false,
                        extra_repos: vec![],
                        max_tokens: None,
                        max_duration_secs: None,
                        max_cost_usd: None,
                        max_nodes: None,
                    };

                    let mut send_result = tokio::time::timeout(
                        Duration::from_secs(30),
                        orbit_client::ipc::send_raw(&req),
                    )
                    .await;

                    // Auto-start daemon on connection failure, then retry once
                    if matches!(&send_result, Ok(Err(_)) | Err(_)) {
                        app.chat.messages.push(chat::ChatMessage::System {
                            text: "Daemon not running — starting…".into(),
                        });
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::process::Command::new(&exe)
                                .args(["daemon", "serve"])
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .stdin(std::process::Stdio::null())
                                .spawn();
                        }
                        tokio::time::sleep(Duration::from_millis(800)).await;
                        send_result = tokio::time::timeout(
                            Duration::from_secs(30),
                            orbit_client::ipc::send_raw(&req),
                        )
                        .await;
                    }

                    match send_result {
                        Ok(Ok(Response::PlanCreated { id, nodes, .. })) => {
                            let stream_rx = if !dry_run {
                                orbit_client::ipc::stream_plan(&id).await.ok()
                            } else {
                                None
                            };
                            chat::apply_plan_created(&mut app.chat, id, nodes, dry_run, stream_rx);
                        }
                        Ok(Ok(Response::Error { message })) => {
                            chat::apply_plan_error(&mut app.chat, message);
                        }
                        Ok(Err(e)) => {
                            chat::apply_plan_error(&mut app.chat, format!("IPC error: {e}"));
                        }
                        Err(_) => {
                            chat::apply_plan_error(&mut app.chat, "Plan creation timed out.".into());
                        }
                        _ => {
                            chat::apply_plan_error(&mut app.chat, "Unexpected response.".into());
                        }
                    }
                }
                chat::ChatAsyncAction::ApprovePlanNode { plan_id, node_id } => {
                    app.pending_async = Some(AsyncAction::ChatApprove { plan_id, node_id });
                }
                chat::ChatAsyncAction::CancelPlan(id) => {
                    app.pending_async = Some(AsyncAction::ChatCancel(id));
                }
                chat::ChatAsyncAction::ListPlans => {
                    app.pending_async = Some(AsyncAction::ChatListPlans);
                }
            }
        }

        AsyncAction::ChatApprove { plan_id, node_id } => {
            match tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::approve_plan_node(&plan_id, &node_id),
            )
            .await
            {
                Ok(Ok(())) => {
                    app.chat.push_orbit(chat::OrbitContent::Text(format!("Approved {node_id}.")));
                }
                Ok(Err(e)) => {
                    app.chat.push_orbit(chat::OrbitContent::Error(format!("Approve failed: {e}")));
                }
                _ => {
                    app.chat.push_orbit(chat::OrbitContent::Error("Approve timed out.".into()));
                }
            }
        }

        AsyncAction::ChatCancel(id) => {
            let id = id.or_else(|| {
                app.chat.active_plan.as_ref().map(|ap| ap.plan_id.clone())
            });
            if let Some(id) = id {
                match tokio::time::timeout(
                    Duration::from_millis(500),
                    orbit_client::ipc::cancel_plan(&id),
                )
                .await
                {
                    Ok(Ok(())) => {
                        app.chat.active_plan = None;
                        app.chat.push_orbit(chat::OrbitContent::Text(format!("Plan {id} cancelled.")));
                    }
                    Ok(Err(e)) => {
                        app.chat.push_orbit(chat::OrbitContent::Error(format!("Cancel failed: {e}")));
                    }
                    _ => {
                        app.chat.push_orbit(chat::OrbitContent::Error("Cancel timed out.".into()));
                    }
                }
            } else {
                app.chat.push_orbit(chat::OrbitContent::Error("No active plan to cancel.".into()));
            }
        }

        AsyncAction::ChatListPlans => {
            if let Ok(Ok(plans)) = tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::list_plans(),
            )
            .await
            {
                let text = chat::format_plan_list(&plans);
                app.chat.push_orbit(chat::OrbitContent::Text(text));
            } else {
                app.chat.push_orbit(chat::OrbitContent::Error("Could not list plans.".into()));
            }
        }

        AsyncAction::DaemonStart => {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(&exe)
                    .args(["daemon", "serve"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .stdin(std::process::Stdio::null())
                    .spawn();
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
            app.sys.refresh();
            app.status_msg = Some("Daemon started.".into());
        }
        AsyncAction::DaemonStop => match orbit_client::ipc::shutdown().await {
            Ok(()) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                app.sys.refresh();
                app.status_msg = Some("Daemon stopped.".into());
            }
            Err(e) => {
                app.status_msg = Some(format!("Stop failed: {e}"));
                app.sys.refresh();
            }
        },
        AsyncAction::RefreshPlans => {
            if let Ok(Ok(plans)) =
                tokio::time::timeout(Duration::from_millis(500), orbit_client::ipc::list_plans())
                    .await
            {
                let n = plans.len();
                app.plans.plans = plans;
                app.plans.selected = app.plans.selected.min(n.saturating_sub(1));
                app.plans.table_state.select(if n > 0 {
                    Some(app.plans.selected)
                } else {
                    None
                });
                app.scopes.refresh(&app.plans.plans);
            }
        }

        AsyncAction::CancelPlan(id) => {
            match tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::cancel_plan(&id),
            )
            .await
            {
                Ok(Ok(())) => {
                    app.status_msg = Some(format!("Plan {id} cancelled."));
                    app.pending_async = Some(AsyncAction::RefreshPlans);
                }
                Ok(Err(e)) => {
                    app.status_msg = Some(format!("Cancel failed: {e}"));
                }
                _ => {
                    app.status_msg = Some("Cancel timed out.".into());
                }
            }
        }

        AsyncAction::RefreshTasks => {
            let workspace = {
                let cfg = orbit_core::user_config::UserConfig::load();
                let root = cfg.ai_root_expanded();
                root.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "AI".into())
            };

            // Load from the task store (kept fresh by the daemon poller).
            let stored = orbit_core::task::load_for_tui(&workspace);
            let n = stored.len();
            app.tasks.items = stored;
            app.tasks.loaded = true;
            app.tasks.loading = false;
            app.tasks.error = None;
            app.tasks.last_store_mtime = orbit_core::task::store_mtime(&workspace);
            app.tasks.selected = 0;
            app.tasks
                .table_state
                .select(if n > 0 { Some(0) } else { None });
        }

        AsyncAction::ForceRefreshTasks => {
            app.tasks.loading = true;
            app.tasks.error = None;

            let result = tokio::task::spawn_blocking(move || {
                let cfg = orbit_core::user_config::UserConfig::load();
                let root = cfg.ai_root_expanded();
                let workspace = root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "AI".into());

                // If Jira is installed, do a live fetch (writes to task store).
                let jira_installed = orbit_core::plugin::load_all()
                    .iter()
                    .find(|p| p.name == "jira")
                    .map(|p| p.is_installed())
                    .unwrap_or(false);

                if jira_installed {
                    let orgs = orbit_core::jira::load_orgs();
                    if !orgs.is_empty() {
                        let issues = orbit_core::jira::fetch_issues(&orgs);
                        for issue in &issues {
                            let orbit_task = issue.to_orbit_task(&workspace, None, None, None);
                            let url = match &orbit_task.source {
                                orbit_core::task::TaskSource::Plugin { url, .. } => url.clone(),
                                _ => None,
                            };
                            let data = orbit_core::task::UpsertData {
                                title: orbit_task.title,
                                status: orbit_task.status,
                                priority: orbit_task.priority,
                                task_type: orbit_task.task_type,
                                url,
                                tenant: None,
                                project: None,
                                repository: None,
                            };
                            let _ = orbit_core::task::upsert_by_external_id(
                                &workspace, "jira", &issue.key, data,
                            );
                        }
                    }
                }

                (orbit_core::task::load_for_tui(&workspace), workspace)
            })
            .await;

            app.tasks.loading = false;
            app.tasks.loaded = true;
            match result {
                Ok((items, workspace)) => {
                    let n = items.len();
                    app.tasks.last_store_mtime = orbit_core::task::store_mtime(&workspace);
                    app.tasks.items = items;
                    app.tasks.selected = 0;
                    app.tasks
                        .table_state
                        .select(if n > 0 { Some(0) } else { None });
                    app.tasks.error = None;
                }
                Err(e) => {
                    app.tasks.error = Some(format!("Refresh failed: {e}"));
                }
            }
        }

        AsyncAction::FetchTaskDetail(key) => {
            let key_clone = key.clone();
            let result = tokio::task::spawn_blocking(move || {
                orbit_core::jira::fetch_issue_detail(&key_clone)
            })
            .await;

            app.mode = match result {
                Ok(Ok(detail)) => Mode::TaskDetails(Box::new(detail)),
                Ok(Err(msg)) => Mode::TaskDetailsError(msg),
                Err(e) => Mode::TaskDetailsError(format!("Task error: {e}")),
            };
        }

        AsyncAction::AddComment { key, body } => {
            let (k, b) = (key.clone(), body.clone());
            let result =
                tokio::task::spawn_blocking(move || orbit_core::jira::add_comment(&k, &b)).await;
            app.status_msg = Some(match result {
                Ok(Ok(())) => format!("Comment added to {key}"),
                Ok(Err(e)) => format!("Error: {e}"),
                Err(e) => format!("Error: {e}"),
            });
        }

        AsyncAction::ApprovePlanNode { plan_id, node_id } => {
            match tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::approve_plan_node(&plan_id, &node_id),
            )
            .await
            {
                Ok(Ok(())) => {
                    app.status_msg = Some(format!("Approved node {node_id}."));
                    app.pending_async = Some(AsyncAction::RefreshPlans);
                }
                Ok(Err(e)) => {
                    app.status_msg = Some(format!("Approve failed: {e}"));
                }
                _ => {
                    app.status_msg = Some("Approve timed out.".into());
                }
            }
        }

        AsyncAction::RefreshSchedules => {
            if let Ok(Ok(scheds)) = tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::list_schedules(),
            )
            .await
            {
                let n = scheds.len();
                app.schedules.schedules = scheds;
                app.schedules.selected = app.schedules.selected.min(n.saturating_sub(1));
                app.schedules.table_state.select(if n > 0 {
                    Some(app.schedules.selected)
                } else {
                    None
                });
            }
        }

        AsyncAction::CancelSchedule(id) => {
            match tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::cancel_schedule(&id),
            )
            .await
            {
                Ok(Ok(())) => {
                    app.status_msg = Some(format!("Schedule {id} cancelled."));
                    app.pending_async = Some(AsyncAction::RefreshSchedules);
                }
                Ok(Err(e)) => {
                    app.status_msg = Some(format!("Cancel failed: {e}"));
                }
                _ => {
                    app.status_msg = Some("Cancel timed out.".into());
                }
            }
        }

        AsyncAction::StartSharing { role, port, name } => {
            use orbit_core::ipc::{ProjectRole, Request, Response};
            let max_role = match role {
                ShareRole::Observer => ProjectRole::Observer,
                ShareRole::Contributor => ProjectRole::Contributor,
            };
            match tokio::time::timeout(
                Duration::from_secs(5),
                orbit_client::ipc::send_raw(&Request::StartServing {
                    port,
                    max_role,
                    name: name.clone(),
                }),
            )
            .await
            {
                Ok(Ok(Response::ServingStarted { port, .. })) => {
                    app.serving_active = true;
                    app.status_msg = Some(format!("Sharing on port {port} via mDNS as '{name}'"));
                }
                Ok(Ok(Response::Error { message })) => {
                    app.status_msg = Some(format!("Serve error: {message}"));
                }
                Ok(Err(e)) => {
                    app.status_msg = Some(format!("Serve error: {e}"));
                }
                _ => {
                    app.status_msg = Some("Serve timed out.".into());
                }
            }
        }

        AsyncAction::StopSharing => {
            use orbit_core::ipc::Request;
            let _ = tokio::time::timeout(
                Duration::from_secs(3),
                orbit_client::ipc::send_raw(&Request::StopServing),
            )
            .await;
            app.serving_active = false;
            app.status_msg = Some("Sharing stopped.".into());
        }

        AsyncAction::RefreshPeers => {
            use orbit_core::ipc::{Request, Response};
            if let Ok(Ok(Response::NetworkPeers { peers })) = tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::send_raw(&Request::ListNetworkPeers),
            )
            .await
            {
                app.peers_state.peers = peers;
            }
        }

        AsyncAction::RunScheduleNow(id) => {
            match tokio::time::timeout(
                Duration::from_millis(500),
                orbit_client::ipc::send_raw(&orbit_core::ipc::Request::RunScheduleNow {
                    id: id.clone(),
                }),
            )
            .await
            {
                Ok(Ok(_)) => {
                    app.status_msg = Some(format!("Schedule {id} fired."));
                    app.pending_async = Some(AsyncAction::RefreshSchedules);
                }
                Ok(Err(e)) => {
                    app.status_msg = Some(format!("Run failed: {e}"));
                }
                _ => {
                    app.status_msg = Some("Run timed out.".into());
                }
            }
        }
    }
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<Option<PostAction>>
where
    B::Error: Send + Sync + 'static,
{
    let mut last_refresh = Instant::now();

    loop {
        // Drain chat stream events before rendering so the frame shows latest state.
        app.chat.drain_stream();

        terminal.draw(|f| crate::views::render(f, app))?;

        if let Some(action) = app.pending_async.take() {
            handle_async_action(action, app).await;
        }

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.status_msg = None;
            app.handle_key(key.code, key.modifiers);
        }

        if last_refresh.elapsed() > Duration::from_secs(2) {
            if app.sys.daemon_running {
                match tokio::time::timeout(
                    Duration::from_millis(500),
                    orbit_client::ipc::list_sessions(),
                )
                .await
                {
                    Ok(Ok(sessions)) => {
                        let selected = app.table_state.selected();
                        app.sessions = sessions;
                        if app.sessions.is_empty() {
                            app.table_state.select(None);
                        } else {
                            let i = selected.unwrap_or(0).min(app.sessions.len() - 1);
                            app.table_state.select(Some(i));
                        }
                    }
                    _ => app.refresh_sessions(),
                }

                if (app.tab == Tab::Plans || app.tab == Tab::Scopes)
                    && let Ok(Ok(plans)) = tokio::time::timeout(
                        Duration::from_millis(500),
                        orbit_client::ipc::list_plans(),
                    )
                    .await
                {
                    let n = plans.len();
                    app.plans.plans = plans;
                    app.plans.selected = app.plans.selected.min(n.saturating_sub(1));
                    app.plans.table_state.select(if n > 0 {
                        Some(app.plans.selected)
                    } else {
                        None
                    });
                    app.scopes.refresh(&app.plans.plans);
                }

                if app.tab == Tab::Schedules
                    && let Ok(Ok(scheds)) = tokio::time::timeout(
                        Duration::from_millis(500),
                        orbit_client::ipc::list_schedules(),
                    )
                    .await
                {
                    let n = scheds.len();
                    app.schedules.schedules = scheds;
                    app.schedules.selected = app.schedules.selected.min(n.saturating_sub(1));
                    app.schedules.table_state.select(if n > 0 {
                        Some(app.schedules.selected)
                    } else {
                        None
                    });
                }
            } else {
                app.refresh_sessions();
            }

            // Watch task store for changes written by the daemon poller.
            {
                let workspace = {
                    let cfg = orbit_core::user_config::UserConfig::load();
                    let root = cfg.ai_root_expanded();
                    root.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "AI".into())
                };
                let mtime = orbit_core::task::store_mtime(&workspace);
                if mtime.is_some() && mtime != app.tasks.last_store_mtime {
                    app.tasks.last_store_mtime = mtime;
                    let fresh = orbit_core::task::load_for_tui(&workspace);
                    if !fresh.is_empty() {
                        let n = fresh.len();
                        let sel = app.tasks.selected.min(n.saturating_sub(1));
                        app.tasks.items = fresh;
                        app.tasks.loaded = true;
                        app.tasks.loading = false;
                        app.tasks.selected = sel;
                        app.tasks
                            .table_state
                            .select(if n > 0 { Some(sel) } else { None });
                    }
                }
            }

            last_refresh = Instant::now();
        }

        if app.should_quit {
            return Ok(app.post_action.take());
        }
    }
}

// ── public entry point ────────────────────────────────────────────────────────

pub async fn run() -> Result<Option<LaunchParams>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app).await;

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    match result? {
        Some(PostAction::Attach(session)) => {
            attach_to_session(&session)?;
            Ok(None)
        }
        Some(PostAction::Launch(params)) => Ok(Some(params)),
        None => Ok(None),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

pub fn attach_to_session(session: &Session) -> Result<()> {
    let tmux_name = session
        .tmux_session
        .as_deref()
        .expect("attach called on session without tmux");

    let cmd = if std::env::var("TMUX").is_ok() {
        "switch-client"
    } else {
        "attach-session"
    };

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new("tmux")
            .args([cmd, "-t", tmux_name])
            .exec();
        anyhow::bail!("Failed to exec tmux: {err}");
    }

    #[cfg(not(unix))]
    anyhow::bail!("tmux attach is only supported on Unix");
}

pub fn send_sigterm(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn dirs_home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn hostname_or_orbit() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "orbit".to_string())
}
