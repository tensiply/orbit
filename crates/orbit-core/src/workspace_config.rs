use serde::{Deserialize, Serialize};
use std::path::Path;

// ── WorkspaceConfig ───────────────────────────────────────────────────────────

/// Workspace-level config stored in `<ai_root>/orbit.toml`.
/// Owned and distributed by the company through the governance repo.
/// Users never edit this directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub governance: GovernanceSection,
    pub update: UpdateSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GovernanceSection {
    /// Git URL of the governance repository.
    pub url: String,
    /// Pull governance configs automatically on launch.
    pub auto_sync: bool,
    /// Minimum hours between auto-syncs (0 = every launch).
    pub sync_interval_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSection {
    /// Internal URL to download new orbit binaries.
    /// Format: `http://server/orbit/latest/{platform}` where platform is
    /// `linux-x86_64`, `linux-aarch64`, `darwin-x86_64`, `darwin-aarch64`.
    pub binary_url: String,
    /// Check for a newer orbit release on startup (default: true).
    /// Set to false to opt out team-wide. Use ORBIT_NO_UPDATE_CHECK=1 for per-user opt-out.
    pub check_on_startup: bool,
}

impl Default for UpdateSection {
    fn default() -> Self {
        Self {
            binary_url: String::new(),
            check_on_startup: true,
        }
    }
}

// ── load ──────────────────────────────────────────────────────────────────────

impl WorkspaceConfig {
    pub fn load(ai_root: &Path) -> Self {
        let path = ai_root.join("orbit.toml");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// `true` when a governance URL has been configured.
    pub fn has_governance(&self) -> bool {
        !self.governance.url.is_empty()
    }

    /// `true` when a binary update URL has been configured.
    pub fn has_binary_url(&self) -> bool {
        !self.update.binary_url.is_empty()
    }

    /// Resolve the binary download URL for a specific release tag and channel.
    ///
    /// Asset names embed channel + version (`orbit-{channel}-{version}-{os}-{arch}`),
    /// so downloads must target the exact tag — the `latest` alias can't reference
    /// a versioned filename. A custom `binary_url` base is honored as the directory.
    pub fn binary_url_for_tag(&self, tag: &str, channel: &str) -> Option<String> {
        let version = tag.trim_start_matches('v');
        let asset = orbit_asset_name(channel, version);
        if !self.update.binary_url.is_empty() {
            let base = self.update.binary_url.trim_end_matches('/');
            Some(format!("{base}/{asset}"))
        } else {
            Some(format!(
                "https://github.com/tensiply/orbit/releases/download/{tag}/{asset}"
            ))
        }
    }
}

/// Canonical release asset name: `orbit-{channel}-{version}-{os}-{arch}`
/// (e.g. `orbit-canary-0.22.1-linux-x86_64`). The single source of truth for the
/// artifact contract, shared by the self-updater and produced verbatim by CI.
pub fn orbit_asset_name(channel: &str, version: &str) -> String {
    format!("orbit-{channel}-{version}-{}", current_platform())
}

/// Scan `home` for direct subdirectories that look like orbit workspaces.
/// A workspace directory must contain `orbit.toml` or a `tenants/` subdirectory.
pub fn detect_workspaces(home: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return vec![];
    };
    let mut workspaces: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            let p = e.path();
            p.join("orbit.toml").exists() || p.join("tenants").is_dir()
        })
        .map(|e| e.path())
        .collect();
    workspaces.sort();
    workspaces
}

fn current_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("macos", "x86_64") => "macos-x86_64",
        ("macos", "aarch64") => "macos-aarch64",
        _ => "linux-x86_64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_embeds_channel_version_platform() {
        let name = orbit_asset_name("canary", "0.22.1");
        assert!(name.starts_with("orbit-canary-0.22.1-"));
        assert!(name.ends_with(current_platform()));
    }

    #[test]
    fn binary_url_for_tag_targets_exact_tag() {
        let cfg = WorkspaceConfig::default();
        let url = cfg.binary_url_for_tag("v0.22.1", "canary").unwrap();
        assert!(url.contains("/releases/download/v0.22.1/orbit-canary-0.22.1-"));
    }
}
