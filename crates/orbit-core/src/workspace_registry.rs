use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    /// Human-readable name (e.g. "AI", "BeFra").
    pub name: String,
    /// Lowercase slug used for data directory naming (e.g. "ai", "befra").
    pub slug: String,
    /// Path to the AI governance root for this workspace.
    pub ai_root: PathBuf,
    /// Whether this is the default workspace.
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceRegistry {
    #[serde(default)]
    pub workspaces: Vec<WorkspaceEntry>,
}

// ── WorkspaceRegistry ─────────────────────────────────────────────────────────

impl WorkspaceRegistry {
    pub fn path() -> PathBuf {
        config_dir().join("orbit/workspaces.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text)?;
        Ok(())
    }

    /// Look up an entry by name or slug (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&WorkspaceEntry> {
        let needle = name.to_lowercase();
        self.workspaces
            .iter()
            .find(|e| e.name.to_lowercase() == needle || e.slug == needle)
    }

    pub fn default_entry(&self) -> Option<&WorkspaceEntry> {
        self.workspaces
            .iter()
            .find(|e| e.is_default)
            .or_else(|| self.workspaces.first())
    }

    /// Add or update a workspace. Setting `is_default = true` clears the flag on all others.
    pub fn add(&mut self, name: &str, ai_root: PathBuf, is_default: bool) {
        let slug = crate::data_paths::slugify(name);
        if is_default {
            for e in &mut self.workspaces {
                e.is_default = false;
            }
        }
        if let Some(entry) = self.workspaces.iter_mut().find(|e| e.slug == slug) {
            entry.name = name.to_string();
            entry.ai_root = ai_root;
            if is_default {
                entry.is_default = true;
            }
        } else {
            self.workspaces.push(WorkspaceEntry {
                name: name.to_string(),
                slug,
                ai_root,
                is_default,
            });
        }
    }

    /// Remove a workspace by name or slug. Returns `true` if it was found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let needle = name.to_lowercase();
        let before = self.workspaces.len();
        self.workspaces
            .retain(|e| e.name.to_lowercase() != needle && e.slug != needle);
        self.workspaces.len() < before
    }

    /// Returns the slug for a workspace name, or `None` if not registered.
    pub fn slug_for(&self, name: &str) -> Option<String> {
        self.get(name).map(|e| e.slug.clone())
    }

    /// All registered workspace slugs.
    pub fn all_slugs(&self) -> Vec<String> {
        self.workspaces.iter().map(|e| e.slug.clone()).collect()
    }

    /// Set a workspace as the default (clears default from all others).
    pub fn set_default(&mut self, name: &str) -> bool {
        let needle = name.to_lowercase();
        let found = self
            .workspaces
            .iter()
            .any(|e| e.name.to_lowercase() == needle || e.slug == needle);
        if found {
            for e in &mut self.workspaces {
                e.is_default = e.name.to_lowercase() == needle || e.slug == needle;
            }
        }
        found
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".config"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn add_and_get_workspace() {
        let mut reg = WorkspaceRegistry::default();
        reg.add("AI", PathBuf::from("/home/user/AI"), true);
        reg.add("BeFra", PathBuf::from("/home/user/BeFra"), false);

        assert_eq!(reg.workspaces.len(), 2);
        assert!(reg.get("AI").is_some());
        assert!(reg.get("ai").is_some()); // slug lookup
        assert!(reg.get("BeFra").is_some());
        assert!(reg.get("befra").is_some());
    }

    #[test]
    fn default_flag_is_exclusive() {
        let mut reg = WorkspaceRegistry::default();
        reg.add("AI", PathBuf::from("/home/user/AI"), true);
        reg.add("BeFra", PathBuf::from("/home/user/BeFra"), false);

        reg.set_default("BeFra");
        assert!(!reg.get("AI").unwrap().is_default);
        assert!(reg.get("BeFra").unwrap().is_default);
    }

    #[test]
    fn remove_workspace() {
        let mut reg = WorkspaceRegistry::default();
        reg.add("AI", PathBuf::from("/home/user/AI"), true);
        assert!(reg.remove("AI"));
        assert!(reg.workspaces.is_empty());
        assert!(!reg.remove("AI")); // second call returns false
    }

    #[test]
    fn round_trip_toml() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("workspaces.toml");

        let mut reg = WorkspaceRegistry::default();
        reg.add("AI", PathBuf::from("/home/user/AI"), true);
        reg.add("BeFra", PathBuf::from("/home/user/BeFra"), false);
        reg.save_to(&path).unwrap();

        let loaded = WorkspaceRegistry::load_from(&path);
        assert_eq!(loaded.workspaces.len(), 2);
        assert_eq!(loaded.get("AI").unwrap().slug, "ai");
        assert!(loaded.get("AI").unwrap().is_default);
    }
}
