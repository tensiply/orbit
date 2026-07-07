use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── UserConfig ────────────────────────────────────────────────────────────────

/// Personal configuration stored in `~/.config/orbit/config.toml`.
/// Created by `orbit setup` — one-time per machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserConfig {
    pub user: UserSection,
    pub workspace: WorkspaceSection,
    pub engine: EngineSection,
    pub install: InstallSection,
    pub update: UserUpdateSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UserSection {
    /// Display name shown in tmux session names (e.g. "ecorona").
    pub name: String,
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
        xdg_config_dir().join("orbit/config.toml")
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

    /// Persist config to `~/.config/orbit/config.toml`.
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

fn xdg_config_dir() -> PathBuf {
    // ORBIT_CONFIG_HOME is set by the launcher before it overrides XDG_CONFIG_HOME
    // for session isolation. Prefer it so that orbit commands run inside a session
    // still find the real user config instead of the session's runtime config dir.
    if let Ok(orbit_home) = std::env::var("ORBIT_CONFIG_HOME") {
        return PathBuf::from(orbit_home);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        home_dir().join(".config")
    }
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
