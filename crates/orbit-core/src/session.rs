use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

// ── Session ───────────────────────────────────────────────────────────────────

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// `{pid}-{started_at}` — unique per launch
    pub id: String,
    pub pid: u32,
    pub engine: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    pub work_dir: PathBuf,
    /// Unix timestamp (seconds)
    pub started_at: u64,
    pub global_mode: bool,
    /// Set by the server when this entry is loaded from history (not currently
    /// running). Not stored on disk — populated at read time only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_history: bool,
    /// tmux session name, if the engine was launched inside tmux
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pid: u32,
        engine: &str,
        tenant: &str,
        project: &str,
        repository: &str,
        work_dir: PathBuf,
        global_mode: bool,
        tmux_session: Option<String>,
    ) -> Self {
        let started_at = now_secs();
        let id = format!("{pid}-{started_at}");
        Self {
            id,
            pid,
            engine: engine.to_string(),
            tenant: tenant.to_string(),
            project: project.to_string(),
            repository: repository.to_string(),
            work_dir,
            started_at,
            global_mode,
            is_history: false,
            tmux_session,
        }
    }

    /// `true` if this session was launched inside a tmux session.
    pub fn has_tmux(&self) -> bool {
        self.tmux_session.is_some()
    }

    /// `~/.orbit/data/sessions/`
    pub fn sessions_dir() -> PathBuf {
        crate::data_paths::orbit_data_root().join("sessions")
    }

    /// `~/.orbit/data/session-history.ndjson` — append-only session history log.
    pub fn history_path() -> PathBuf {
        crate::data_paths::orbit_data_root().join("session-history.ndjson")
    }

    /// Persist the session file and record it in the history log.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::sessions_dir())?;
        let _ = self.append_to_history(); // best-effort; never fail the launch
        Ok(())
    }

    pub fn save_to(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.json", self.id));
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// One-time migration: seed the history log from existing session files so
    /// sessions launched before history tracking was introduced are visible.
    pub fn seed_history_from_existing() {
        let path = Self::history_path();
        if path.exists() {
            return; // already seeded
        }
        for session in Self::load_all() {
            let _ = session.append_to_history();
        }
    }

    /// Delete the session file.
    pub fn delete(&self) -> Result<()> {
        self.delete_from(&Self::sessions_dir())
    }

    pub fn delete_from(&self, dir: &Path) -> Result<()> {
        let path = dir.join(format!("{}.json", self.id));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Append this session to the history log (NDJSON, one entry per line).
    /// Called once at session creation — history is recorded at launch time,
    /// not at cleanup time, so it survives any deletion path.
    pub fn append_to_history(&self) -> Result<()> {
        use std::io::Write as _;
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", serde_json::to_string(self)?)?;
        Ok(())
    }

    /// Load up to `limit` recent sessions from the history log, excluding IDs
    /// already present in `exclude_ids` (the active sessions).
    /// Returns entries sorted by `started_at` descending (most recent first).
    pub fn load_history_excluding(
        limit: usize,
        exclude_ids: &std::collections::HashSet<String>,
    ) -> Vec<Session> {
        let Ok(content) = fs::read_to_string(Self::history_path()) else {
            return vec![];
        };
        // Deduplicate by id — last line wins (future: could carry ended_at updates).
        let mut by_id: std::collections::HashMap<String, Session> =
            std::collections::HashMap::new();
        for line in content.lines() {
            if let Ok(s) = serde_json::from_str::<Session>(line) {
                by_id.insert(s.id.clone(), s);
            }
        }
        let mut sessions: Vec<Session> = by_id
            .into_values()
            .filter(|s| !exclude_ids.contains(&s.id))
            .collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));
        sessions.truncate(limit);
        sessions
    }

    /// Load all session files from the sessions directory.
    pub fn load_all() -> Vec<Session> {
        Self::load_from(&Self::sessions_dir())
    }

    pub fn load_from(dir: &Path) -> Vec<Session> {
        let Ok(entries) = fs::read_dir(dir) else {
            return vec![];
        };
        let mut sessions: Vec<Session> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| {
                fs::read_to_string(e.path())
                    .ok()
                    .and_then(|text| serde_json::from_str(&text).ok())
            })
            .collect();
        sessions.sort_by_key(|s| s.started_at);
        sessions
    }

    /// `true` if the process is still running.
    pub fn is_running(&self) -> bool {
        is_pid_alive(self.pid)
    }

    /// `true` if the tmux session window still exists (`tmux has-session`).
    /// Always returns `false` when the session was not launched with tmux.
    pub fn tmux_window_exists(&self) -> bool {
        let Some(ref name) = self.tmux_session else {
            return false;
        };
        std::process::Command::new("tmux")
            .args(["has-session", "-t", name])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Human-readable scope label, always prefixed with workspace name.
    pub fn scope_label(&self) -> String {
        if self.global_mode {
            return "(global)".to_string();
        }
        let parts: Vec<&str> = [&self.tenant, &self.project, &self.repository]
            .iter()
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            return "(unknown)".to_string();
        }
        if let Some(ws) = self.workspace_prefix() {
            let mut all = vec![ws.as_str()];
            all.extend_from_slice(&parts);
            return all.join(" > ");
        }
        parts.join(" > ")
    }

    fn workspace_prefix(&self) -> Option<String> {
        let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
        let rel = self.work_dir.strip_prefix(&home).ok()?;
        rel.components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .map(|s| s.to_owned())
    }

    /// Human-readable "Xm ago" / "Xh ago" elapsed time.
    pub fn started_ago(&self) -> String {
        let elapsed = now_secs().saturating_sub(self.started_at);
        if elapsed < 60 {
            format!("{elapsed}s ago")
        } else if elapsed < 3600 {
            format!("{}m ago", elapsed / 60)
        } else {
            format!("{}h ago", elapsed / 3600)
        }
    }
}

// ── OS helpers ────────────────────────────────────────────────────────────────

/// Check if a PID is alive using /proc on Linux.
/// Falls back to `kill -0` on other Unix systems.
pub fn is_pid_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_session(pid: u32) -> Session {
        Session::new(
            pid,
            "opencode",
            "AIDEV",
            "AI-ECOSYSTEM",
            "orbit",
            PathBuf::from("/work"),
            false,
            None,
        )
    }

    #[test]
    fn id_contains_pid() {
        let s = make_session(9999);
        assert!(s.id.starts_with("9999-"));
    }

    #[test]
    fn scope_label_full() {
        let s = make_session(1);
        assert_eq!(s.scope_label(), "AIDEV > AI-ECOSYSTEM > orbit");
    }

    #[test]
    fn scope_label_global_mode() {
        let mut s = make_session(1);
        s.global_mode = true;
        assert_eq!(s.scope_label(), "(global)");
    }

    #[test]
    fn scope_label_partial() {
        let s = Session::new(
            1,
            "claude",
            "AIDEV",
            "AI-ECOSYSTEM",
            "",
            PathBuf::from("/work"),
            false,
            None,
        );
        assert_eq!(s.scope_label(), "AIDEV > AI-ECOSYSTEM");
    }

    #[test]
    fn save_and_load() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();

        let s = make_session(42);
        s.save_to(&dir).unwrap();

        let sessions = Session::load_from(&dir);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].pid, 42);
        assert_eq!(sessions[0].engine, "opencode");
    }

    #[test]
    fn delete_removes_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();

        let s = make_session(43);
        s.save_to(&dir).unwrap();
        assert_eq!(Session::load_from(&dir).len(), 1);
        s.delete_from(&dir).unwrap();
        assert_eq!(Session::load_from(&dir).len(), 0);
    }

    #[test]
    fn current_process_is_alive() {
        assert!(is_pid_alive(std::process::id()));
    }

    #[test]
    fn dead_pid_is_not_alive() {
        // PID 1 is init/systemd — always alive, not a good test
        // PID 999999 very unlikely to exist
        assert!(!is_pid_alive(999_999));
    }
}
