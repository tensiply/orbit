use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
};

const BUILTIN_ENGINE_HOOKS: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_engine_hooks.rs"));

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct EngineHookCatalog {
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub events: Vec<EngineHookEventDef>,
    pub requires_binary: Option<String>,
    #[serde(default)]
    pub scripts: Vec<EngineHookScript>,
}

/// A shell script shipped with an engine hook and installed to disk on `enable`.
#[derive(Debug, Clone, Deserialize)]
pub struct EngineHookScript {
    /// Destination path; `$HOME` is expanded at install time.
    pub path: String,
    /// Whether to set the executable bit (755).
    #[serde(default)]
    pub executable: bool,
    /// Verbatim script content.
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EngineHookEventDef {
    /// Claude Code event name: "Stop", "Notification", "PreToolUse", "PostToolUse", etc.
    pub event: String,
    /// Script path; `$HOME` prefix is expanded at materialization time.
    pub command: String,
    /// Optional tool-name matcher for PreToolUse / PostToolUse events.
    pub matcher: Option<String>,
    #[serde(default)]
    pub is_async: bool,
}

// ── state ─────────────────────────────────────────────────────────────────────

/// Tracks which engine hooks are enabled.
/// Persisted at `~/.config/orbit/engine-hook-state.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EngineHookState {
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl EngineHookState {
    pub fn path() -> PathBuf {
        user_config_dir().join("engine-hook-state.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.iter().any(|n| n == name)
    }

    pub fn enable(&mut self, name: &str) {
        if !self.is_enabled(name) {
            self.enabled.push(name.to_string());
        }
    }

    pub fn disable(&mut self, name: &str) {
        self.enabled.retain(|n| n != name);
    }
}

// ── loader ────────────────────────────────────────────────────────────────────

/// Load all engine hooks: built-ins first, then user hooks (`~/.config/orbit/engine-hooks/`).
/// A user entry with the same name overrides the built-in.
pub fn load_all() -> Vec<EngineHookCatalog> {
    let mut hooks: Vec<EngineHookCatalog> = Vec::new();

    for (name, content) in BUILTIN_ENGINE_HOOKS {
        match toml::from_str::<EngineHookCatalog>(content) {
            Ok(h) => hooks.push(h),
            Err(e) => eprintln!("[orbit] failed to parse builtin engine hook '{name}': {e}"),
        }
    }

    let user_dir = user_config_dir().join("engine-hooks");
    if let Ok(dir) = fs::read_dir(&user_dir) {
        let mut paths: Vec<_> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort_by_key(|e| e.path());

        for entry in paths {
            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let Ok(h) = toml::from_str::<EngineHookCatalog>(&content) else {
                continue;
            };
            hooks.retain(|existing| existing.name != h.name);
            hooks.push(h);
        }
    }

    hooks
}

pub fn find(name: &str) -> Option<EngineHookCatalog> {
    load_all().into_iter().find(|h| h.name == name)
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Expand a leading `$HOME` in a command path to the actual home directory.
pub fn expand_home(s: &str) -> String {
    if let (Some(rest), Some(base)) =
        (s.strip_prefix("$HOME"), directories::BaseDirs::new())
    {
        return format!("{}{rest}", base.home_dir().display());
    }
    s.to_string()
}

/// Install all scripts declared by a hook to their target paths.
/// Returns the list of paths written.
pub fn install_scripts(hook: &EngineHookCatalog) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for script in &hook.scripts {
        let dest = PathBuf::from(expand_home(&script.path));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir for {}", dest.display()))?;
        }
        fs::write(&dest, &script.content)
            .with_context(|| format!("write script {}", dest.display()))?;
        #[cfg(unix)]
        if script.executable {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("chmod +x {}", dest.display()))?;
        }
        written.push(dest);
    }
    Ok(written)
}

fn user_config_dir() -> PathBuf {
    std::env::var("ORBIT_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            directories::BaseDirs::new()
                .map(|b| b.home_dir().join(".config"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        })
        .join("orbit")
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_home_replaces_prefix() {
        let home = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_string_lossy().to_string())
            .unwrap_or_default();
        let result = expand_home("$HOME/.claude/hooks/on-stop.sh");
        assert!(result.starts_with(&home), "should start with home dir");
        assert!(result.ends_with("/.claude/hooks/on-stop.sh"));
    }

    #[test]
    fn expand_home_no_prefix_unchanged() {
        let path = "/usr/local/bin/script.sh";
        assert_eq!(expand_home(path), path);
    }

    #[test]
    fn state_enable_disable_roundtrip() {
        let mut state = EngineHookState::default();
        state.enable("session-logging");
        assert!(state.is_enabled("session-logging"));
        state.disable("session-logging");
        assert!(!state.is_enabled("session-logging"));
    }
}
