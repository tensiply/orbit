use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const BUILTIN_PLUGINS: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_plugins.rs"));

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub description: String,
    pub category: String,
    pub url: Option<String>,
    pub check: CheckSpec,
    #[serde(default)]
    pub install: Vec<InstallMethod>,
    pub auth: Option<AuthSpec>,
    pub wrap: Option<WrapSpec>,
    /// MCP servers contributed by this plugin when enabled.
    #[serde(default)]
    pub mcp: Vec<PluginMcp>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckSpec {
    pub binary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallMethod {
    pub method: String,
    pub cmd: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthSpec {
    pub hint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WrapSpec {
    pub cmd_template: String,
    pub unwrap_cmd_template: Option<String>,
    pub engines: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMcp {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub label: Option<String>,
}

// ── plugin state ──────────────────────────────────────────────────────────────

/// Tracks which plugins are enabled (MCP servers active).
/// Persisted at `~/.config/orbit/plugin-state.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PluginState {
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl PluginState {
    pub fn path() -> PathBuf {
        user_config_dir().join("plugin-state.toml")
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

// ── plugin impl ───────────────────────────────────────────────────────────────

impl Plugin {
    pub fn is_installed(&self) -> bool {
        if let Some(bin) = &self.check.binary {
            return bin_available(bin);
        }
        false
    }

    pub fn has_mcp(&self) -> bool {
        !self.mcp.is_empty()
    }

    /// First install method whose prerequisite tool is available.
    /// Falls back to the first method unconditionally.
    pub fn best_install_method(&self) -> Option<&InstallMethod> {
        for m in &self.install {
            let prereq = match m.method.as_str() {
                "pip" | "pip3" => "pip",
                "npm" => "npm",
                "cargo" => "cargo",
                "brew" => "brew",
                "apt" | "apt-get" => "apt-get",
                _ => continue,
            };
            if bin_available(prereq) {
                return Some(m);
            }
        }
        self.install.first()
    }

    pub fn install_method_by_name(&self, name: &str) -> Option<&InstallMethod> {
        self.install.iter().find(|m| m.method == name)
    }
}

// ── loader ────────────────────────────────────────────────────────────────────

/// Load all plugins: built-ins first, then user plugins (`~/.config/orbit/plugins/`).
/// A user plugin with the same name overrides the built-in.
pub fn load_all() -> Vec<Plugin> {
    let mut plugins: Vec<Plugin> = Vec::new();

    for (_, content) in BUILTIN_PLUGINS {
        if let Ok(p) = toml::from_str::<Plugin>(content) {
            plugins.push(p);
        }
    }

    if let Ok(dir) = fs::read_dir(user_plugins_dir()) {
        let mut paths: Vec<_> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort_by_key(|e| e.path());

        for entry in paths {
            let Ok(content) = fs::read_to_string(entry.path()) else { continue };
            let Ok(p) = toml::from_str::<Plugin>(&content) else { continue };
            plugins.retain(|existing| existing.name != p.name);
            plugins.push(p);
        }
    }

    plugins
}

pub fn find(name: &str) -> Option<Plugin> {
    load_all().into_iter().find(|p| p.name == name)
}

// ── plugins.mcp.json management ───────────────────────────────────────────────

/// Path to the orbit-managed MCP file that holds MCPs for enabled plugins.
pub fn plugins_mcp_path() -> PathBuf {
    user_config_dir().join("plugins.mcp.json")
}

/// Add (or update) this plugin's MCP entries in `plugins.mcp.json`.
pub fn add_plugin_mcps(plugin: &Plugin) -> Result<()> {
    if plugin.mcp.is_empty() {
        return Ok(());
    }
    let path = plugins_mcp_path();
    let mut val = read_plugins_mcp_file(&path);

    let servers = val["mcpServers"]
        .as_object_mut()
        .expect("mcpServers should be an object");

    for entry in &plugin.mcp {
        let mut server = serde_json::json!({
            "command": entry.command,
            "args": entry.args,
        });
        if !entry.env.is_empty() {
            server["env"] = serde_json::to_value(&entry.env)?;
        }
        servers.insert(entry.name.clone(), server);
    }

    write_plugins_mcp_file(&path, &val)
}

/// Remove this plugin's MCP entries from `plugins.mcp.json`.
pub fn remove_plugin_mcps(plugin: &Plugin) -> Result<()> {
    if plugin.mcp.is_empty() {
        return Ok(());
    }
    let path = plugins_mcp_path();
    if !path.is_file() {
        return Ok(());
    }
    let mut val = read_plugins_mcp_file(&path);

    if let Some(servers) = val["mcpServers"].as_object_mut() {
        for entry in &plugin.mcp {
            servers.remove(&entry.name);
        }
    }

    write_plugins_mcp_file(&path, &val)
}

fn read_plugins_mcp_file(path: &Path) -> serde_json::Value {
    if path.is_file() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(empty_mcp_json)
    } else {
        empty_mcp_json()
    }
}

fn write_plugins_mcp_file(path: &Path, val: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(val)?)?;
    Ok(())
}

fn empty_mcp_json() -> serde_json::Value {
    serde_json::json!({ "mcpServers": {} })
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn user_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".config"))
            .unwrap_or_else(|| PathBuf::from("/"))
    }
    .join("orbit")
}

fn user_plugins_dir() -> PathBuf {
    user_config_dir().join("plugins")
}

pub fn bin_available(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
