use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use orbit_core::{
    engine::Engine, ipc::socket_path, session::Session, user_config::UserConfig,
    workspace_config::detect_workspaces,
};
use ratatui::{Terminal, backend::CrosstermBackend, widgets::TableState};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{LaunchParams, widget::TextInput};

// ── tabs ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Sessions,
    Launch,
    System,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Tab::Sessions => Tab::Launch,
            Tab::Launch => Tab::System,
            Tab::System => Tab::Sessions,
        }
    }
}

// ── mode (popup overlays) ─────────────────────────────────────────────────────

pub enum Mode {
    Normal,
    Help,
    ConfirmKill(Session),
    SessionDetails(Session),
    AddMcp(Box<AddMcpState>),
    ConfirmRemoveMcp(crate::mcp::McpEntry),
}

// ── engines ───────────────────────────────────────────────────────────────────

pub const ENGINES: [Engine; 3] = [Engine::Opencode, Engine::Gemini, Engine::Claude];

// ── launch state ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LaunchField {
    Engine,
    Tenant,
    Project,
    Repository,
    NoTmux,
    Launch,
}

impl LaunchField {
    pub fn next(self) -> Self {
        match self {
            LaunchField::Engine => LaunchField::Tenant,
            LaunchField::Tenant => LaunchField::Project,
            LaunchField::Project => LaunchField::Repository,
            LaunchField::Repository => LaunchField::NoTmux,
            LaunchField::NoTmux => LaunchField::Launch,
            LaunchField::Launch => LaunchField::Engine,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            LaunchField::Engine => LaunchField::Launch,
            LaunchField::Tenant => LaunchField::Engine,
            LaunchField::Project => LaunchField::Tenant,
            LaunchField::Repository => LaunchField::Project,
            LaunchField::NoTmux => LaunchField::Repository,
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
    pub tenant: TextInput,
    pub project: TextInput,
    pub repository: TextInput,
    pub no_tmux: bool,
    pub focused: LaunchField,
}

impl LaunchState {
    pub fn new(default_tenant: &str, default_engine: &str) -> Self {
        let engine_idx = ENGINES
            .iter()
            .position(|e| e.as_str() == default_engine)
            .unwrap_or(0);

        Self {
            engine_idx,
            tenant: TextInput::new("Tenant", "AIDEV").with_value(default_tenant),
            project: TextInput::new("Project", "my-project"),
            repository: TextInput::new("Repo", "(optional)"),
            no_tmux: false,
            focused: LaunchField::Engine,
        }
    }

    pub fn engine(&self) -> Engine {
        ENGINES[self.engine_idx]
    }

    pub fn to_params(&self) -> LaunchParams {
        LaunchParams {
            engine: self.engine(),
            tenant: self.tenant.as_str().to_string(),
            project: self.project.as_str().to_string(),
            repository: self.repository.as_str().to_string(),
            no_tmux: self.no_tmux,
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
    DaemonStart,
    DaemonStop,
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
    pub sessions: Vec<Session>,
    pub table_state: TableState,
    pub launch: LaunchState,
    pub sys: SystemState,
    pub status_msg: Option<String>,
    pub pending_async: Option<AsyncAction>,
    pub workspaces: Vec<PathBuf>,
    pub workspace_idx: usize,
    should_quit: bool,
    post_action: Option<PostAction>,
}

impl App {
    fn new() -> Self {
        let sys = SystemState::load();
        let launch = LaunchState::new(&sys.default_tenant, &sys.default_engine);

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
            tab: Tab::Sessions,
            mode: Mode::Normal,
            sessions,
            table_state,
            launch,
            sys,
            status_msg: None,
            pending_async: None,
            workspaces,
            workspace_idx,
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

        // ? always opens help (except when typing in Launch tab)
        if code == KeyCode::Char('?') && !self.launch.focused.is_text_input() {
            self.mode = Mode::Help;
            return;
        }

        // Tab switching + workspace cycling: skip when typing in a text field
        let in_text_input = self.tab == Tab::Launch && self.launch.focused.is_text_input();
        if !in_text_input {
            match code {
                KeyCode::Tab | KeyCode::BackTab => {
                    self.tab = self.tab.next();
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
                    self.tab = Tab::System;
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
            Tab::Sessions => self.handle_sessions_key(code),
            Tab::Launch => self.handle_launch_key(code),
            Tab::System => self.handle_system_key(code),
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
                let n = ENGINES.len();
                self.launch.engine_idx = (self.launch.engine_idx + n - 1) % n;
            }
            KeyCode::Right => {
                self.launch.engine_idx = (self.launch.engine_idx + 1) % ENGINES.len();
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
}

// ── event loop ────────────────────────────────────────────────────────────────

async fn handle_async_action(action: AsyncAction, app: &mut App) {
    match action {
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
    }
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<Option<PostAction>> {
    let mut last_refresh = Instant::now();

    loop {
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
            } else {
                app.refresh_sessions();
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
