use crate::notify::NotificationsConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── UserConfig ────────────────────────────────────────────────────────────────

/// Personal configuration stored in `~/.orbit/config.toml`.
/// Created by `orbit setup` — one-time per machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserConfig {
    pub user: UserSection,
    pub workspace: WorkspaceSection,
    pub engine: EngineSection,
    pub install: InstallSection,
    pub update: UserUpdateSection,
    pub notifications: NotificationsConfig,
    pub budget: BudgetConfig,
    pub plan_retention: PlanRetentionConfig,
    pub planner: PlannerSection,
}

/// Default budget limits applied to every new plan unless overridden at creation time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BudgetConfig {
    /// Maximum total tokens a plan may spend across all nodes (None = unlimited).
    pub max_tokens: Option<u64>,
    /// Maximum wall-clock seconds a plan may run before it is hard-stopped (None = unlimited).
    pub max_duration_secs: Option<u64>,
    /// Maximum estimated USD cost a plan may accumulate (None = unlimited).
    pub max_cost_usd: Option<f64>,
    /// Maximum number of nodes a plan may dispatch (None = unlimited).
    pub max_nodes: Option<u32>,
}

/// Retention policy for completed/failed/cancelled plans.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanRetentionConfig {
    /// Automatically prune old terminal plans in the background (default: false).
    pub auto_prune_enabled: bool,
    /// Age in days after which terminal plans are pruned (default: 30).
    pub auto_prune_days: u32,
    /// Move pruned plans to an archive directory instead of deleting them (default: true).
    pub archive_on_prune: bool,
}

impl Default for PlanRetentionConfig {
    fn default() -> Self {
        Self {
            auto_prune_enabled: false,
            auto_prune_days: 30,
            archive_on_prune: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UserSection {
    /// Display name shown in tmux session names (e.g. "ecorona").
    pub name: String,
    /// Full display name for documents and reports (e.g. "Eloir Corona").
    /// Falls back to `name` when empty.
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserUpdateSection {
    /// Pull the governance repo automatically in background on every invocation.
    pub auto_update_governance: bool,
    /// Download and install a new orbit binary in background when one is available.
    pub auto_update_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceSection {
    /// Root of the AI workspace (governance repo lives here).
    pub ai_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSection {
    /// Default AI engine when none is specified on the CLI.
    pub default: String,
    /// Default tenant when none is specified on the CLI.
    pub default_tenant: String,
    /// Default workspace name when none is specified on the CLI.
    pub default_workspace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallSection {
    /// Directory where the orbit binary is installed.
    pub dir: PathBuf,
}

/// Engine + optional model override for a specific fixed activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ActivityEngineConfig {
    /// Engine to use (e.g. "Claude", "Gemini"). Defaults to "Claude".
    pub engine: String,
    /// Optional model override (e.g. "gemini-2.5-flash"). None = engine default.
    pub model: Option<String>,
}

impl Default for ActivityEngineConfig {
    fn default() -> Self {
        Self {
            engine: "Claude".to_string(),
            model: None,
        }
    }
}

/// Per-activity engine configuration. Controls which engine/model runs each
/// fixed planner activity (scope detection, gap resolution, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ActivitiesConfig {
    /// Scope detection from intent (should be cheap/fast).
    pub scope_detection: Option<ActivityEngineConfig>,
    /// Gap resolution before planning (should be cheap/fast).
    pub gap_resolution: Option<ActivityEngineConfig>,
    /// Main plan generation (benefits from a capable model).
    pub plan_generation: Option<ActivityEngineConfig>,
    /// Node validation after planning (balance of speed and accuracy).
    pub node_validation: Option<ActivityEngineConfig>,
}

/// Planner behaviour settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlannerSection {
    /// Maximum validation+retry cycles before giving up (default: 3).
    pub validation_retries: u8,
    /// Skip the gap-resolution phase entirely (default: false).
    pub skip_gap_resolution: bool,
    /// Per-activity engine overrides.
    pub activities: ActivitiesConfig,
}

impl Default for PlannerSection {
    fn default() -> Self {
        Self {
            validation_retries: 3,
            skip_gap_resolution: false,
            activities: ActivitiesConfig::default(),
        }
    }
}

// ── defaults ──────────────────────────────────────────────────────────────────

impl Default for UserUpdateSection {
    fn default() -> Self {
        Self {
            auto_update_governance: true,
            auto_update_binary: true,
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for UserConfig {
    fn default() -> Self {
        Self {
            user: UserSection::default(),
            workspace: WorkspaceSection::default(),
            engine: EngineSection::default(),
            install: InstallSection::default(),
            update: UserUpdateSection::default(),
            notifications: NotificationsConfig::default(),
            budget: BudgetConfig::default(),
            plan_retention: PlanRetentionConfig::default(),
            planner: PlannerSection::default(),
        }
    }
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        Self {
            ai_root: home_dir().join("AI"),
        }
    }
}

impl Default for EngineSection {
    fn default() -> Self {
        Self {
            default: "opencode".to_string(),
            default_tenant: String::new(),
            default_workspace: String::new(),
        }
    }
}

impl Default for InstallSection {
    fn default() -> Self {
        Self {
            dir: home_dir().join(".local/bin"),
        }
    }
}

// ── load / save ───────────────────────────────────────────────────────────────

impl UserConfig {
    /// Returns the path to the user config file.
    pub fn path() -> PathBuf {
        orbit_config_dir().join("config.toml")
    }

    /// Load config from disk. Returns defaults if the file does not exist.
    pub fn load() -> Self {
        let path = Self::path();
        Self::load_from(&path).unwrap_or_default()
    }

    /// Load from an explicit path (useful in tests).
    pub fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&text)?;
        Ok(cfg)
    }

    /// Persist config to `~/.orbit/config.toml`.
    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text)?;
        Ok(())
    }

    /// `ai_root` with `~` expanded to the real home directory.
    pub fn ai_root_expanded(&self) -> PathBuf {
        expand_tilde(&self.workspace.ai_root)
    }

    /// `install.dir` with `~` expanded.
    pub fn install_dir_expanded(&self) -> PathBuf {
        expand_tilde(&self.install.dir)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn orbit_config_dir() -> PathBuf {
    // ORBIT_CONFIG_HOME overrides the default for testing or CI.
    if let Ok(h) = std::env::var("ORBIT_CONFIG_HOME") {
        return PathBuf::from(h);
    }
    home_dir().join(".orbit")
}

/// Replace a leading `~` with the real home directory path.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix("~/") {
        home_dir().join(stripped)
    } else if s == "~" {
        home_dir()
    } else {
        path.to_path_buf()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn defaults_are_sane() {
        let cfg = UserConfig::default();
        assert_eq!(cfg.engine.default, "opencode");
        assert!(cfg.workspace.ai_root.ends_with("AI"));
        assert!(cfg.install.dir.ends_with(".local/bin"));
    }

    #[test]
    fn roundtrip_toml() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");

        let mut cfg = UserConfig::default();
        cfg.workspace.ai_root = PathBuf::from("/custom/ai");
        cfg.engine.default = "claude".to_string();
        cfg.engine.default_tenant = "MYCO".to_string();

        let text = toml::to_string_pretty(&cfg).unwrap();
        fs::write(&path, &text).unwrap();

        let loaded = UserConfig::load_from(&path).unwrap();
        assert_eq!(loaded.workspace.ai_root, PathBuf::from("/custom/ai"));
        assert_eq!(loaded.engine.default, "claude");
        assert_eq!(loaded.engine.default_tenant, "MYCO");
    }

    #[test]
    fn missing_file_returns_defaults() {
        let cfg = UserConfig::load_from(Path::new("/nonexistent/path/config.toml"));
        // load_from errors, but load() (which calls load_from) falls back to default
        assert!(cfg.is_err());
        let default = UserConfig::default();
        assert_eq!(default.engine.default, "opencode");
    }

    #[test]
    fn expand_tilde_works() {
        let home = home_dir();
        let expanded = expand_tilde(Path::new("~/AI"));
        assert_eq!(expanded, home.join("AI"));

        let abs = Path::new("/absolute/path");
        assert_eq!(expand_tilde(abs), abs);
    }
}
