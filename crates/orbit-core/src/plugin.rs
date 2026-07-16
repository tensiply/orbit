use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const BUILTIN_PLUGINS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/builtin_plugins.rs"));

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
    /// TUI tab contributed by this plugin when enabled.
    pub tui: Option<TuiSpec>,
    /// Static context (prompt + instruction files) injected at every session launch.
    pub context: Option<ContextSpec>,
    /// Command run before launching a session; output optionally injected as context.
    pub pre_launch: Option<PreLaunchSpec>,
    /// When present, this plugin can be used as a plan node executor.
    #[serde(default)]
    pub executor: Option<ExecutorSpec>,
    /// When true, Python-based install/check/MCP use the orbit-managed venv
    /// at `~/.local/share/orbit/venv/` instead of the system Python environment.
    #[serde(default)]
    pub use_orbit_venv: bool,
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
    /// When true, wrap is applied automatically on `orbit plugins enable`.
    #[serde(default)]
    pub auto_wrap: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TuiSpec {
    pub tab_title: String,
    #[serde(default)]
    pub can_be_primary: bool,
    pub data_cmd: String,
    #[serde(default = "default_data_refresh_secs")]
    pub data_refresh_secs: u64,
    pub scope_key: String,
}

fn default_data_refresh_secs() -> u64 {
    300
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextSpec {
    pub prompt: Option<String>,
    #[serde(default)]
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreLaunchSpec {
    pub cmd: String,
    /// How to use stdout: "context" | "env" | "none"
    #[serde(default = "default_output_mode")]
    pub output: String,
    pub timeout_secs: Option<u64>,
    pub cache_ttl_secs: Option<u64>,
}

fn default_output_mode() -> String {
    "none".to_string()
}

/// Executor specification: when present the plugin can run plan nodes as an
/// external process instead of an AI engine.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutorSpec {
    /// Command template. Tokens containing `{param_name}` are substituted with
    /// the provided (or default) parameter value. Empty tokens after substitution
    /// are dropped from the final command.
    pub command: Vec<String>,
    #[serde(default)]
    pub params: Vec<ExecutorParam>,
}

/// A named parameter accepted by an executor plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutorParam {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Option<String>,
    /// When true, the executor errors if no value is provided and there is no default.
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMcp {
    pub name: String,
    /// Local binary command. Empty for remote MCPs.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub label: Option<String>,
    /// Remote endpoint URL. When set, the MCP is remote (no local process).
    pub url: Option<String>,
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
            if self.use_orbit_venv {
                return crate::venv::venv_bin(bin).exists();
            }
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
                // venv-based plugins need python3, not pip (pip lives inside the venv)
                "pip" | "pip3" if self.use_orbit_venv => "python3",
                "pip" | "pip3" => "pip",
                "npm" => "npm",
                "cargo" => "cargo",
                "brew" => "brew",
                "apt" | "apt-get" => "apt-get",
                "rustup" => "rustup",
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

    /// Render the executor command by substituting `{param_name}` placeholders
    /// with values from `params` (falling back to declared defaults). Required
    /// params without a value or default produce an error. Empty tokens after
    /// substitution are dropped.
    pub fn render_executor_command(
        &self,
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<String>> {
        let spec = self
            .executor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("plugin '{}' has no [executor] spec", self.name))?;

        for p in &spec.params {
            if p.required && !params.contains_key(&p.name) && p.default.is_none() {
                anyhow::bail!(
                    "executor plugin '{}': required param '{}' not provided",
                    self.name,
                    p.name
                );
            }
        }

        let rendered: Vec<String> = spec
            .command
            .iter()
            .map(|token| {
                let mut t = token.clone();
                for p in &spec.params {
                    let value = params
                        .get(&p.name)
                        .map(|v| v.as_str())
                        .or(p.default.as_deref())
                        .unwrap_or("");
                    t = t.replace(&format!("{{{}}}", p.name), value);
                }
                t
            })
            .filter(|t| !t.is_empty())
            .collect();

        if rendered.is_empty() {
            anyhow::bail!("executor plugin '{}': rendered command is empty", self.name);
        }

        Ok(rendered)
    }
}

// ── loader ────────────────────────────────────────────────────────────────────

/// Load all plugins: built-ins first, then user plugins (`~/.config/orbit/plugins/`).
/// A user plugin with the same name overrides the built-in.
pub fn load_all() -> Vec<Plugin> {
    let mut plugins: Vec<Plugin> = Vec::new();

    for (name, content) in BUILTIN_PLUGINS {
        match toml::from_str::<Plugin>(content) {
            Ok(p) => plugins.push(p),
            Err(e) => eprintln!("[orbit] failed to parse builtin plugin '{name}': {e}"),
        }
    }

    if let Ok(dir) = fs::read_dir(user_plugins_dir()) {
        let mut paths: Vec<_> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort_by_key(|e| e.path());

        for entry in paths {
            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let Ok(p) = toml::from_str::<Plugin>(&content) else {
                continue;
            };
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
        let server = if let Some(url) = &entry.url {
            serde_json::json!({ "type": "http", "url": url })
        } else {
            // Resolve MCP command to the absolute venv path so the AI engine can
            // locate the binary regardless of the user's PATH at session time.
            let command = if plugin.use_orbit_venv {
                crate::venv::venv_bin(&entry.command)
                    .to_string_lossy()
                    .to_string()
            } else {
                entry.command.clone()
            };
            let mut s = serde_json::json!({
                "command": command,
                "args": entry.args,
            });
            if !entry.env.is_empty() {
                s["env"] = serde_json::to_value(&entry.env)?;
            }
            s
        };
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

pub fn user_config_dir() -> PathBuf {
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

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_executor_plugin(command: &[&str], params_toml: &str) -> Plugin {
        let params_section = if params_toml.is_empty() {
            String::new()
        } else {
            params_toml.to_string()
        };
        let toml = format!(
            r#"
name = "testplugin"
description = "test"
category = "test"
[check]
binary = "testplugin"
[executor]
command = [{cmd}]
{params}
"#,
            cmd = command
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", "),
            params = params_section,
        );
        toml::from_str(&toml).expect("valid test plugin TOML")
    }

    #[test]
    fn render_basic_substitution() {
        let plugin = make_executor_plugin(
            &["pytest", "{test_path}"],
            r#"
[[executor.params]]
name = "test_path"
default = "."
"#,
        );
        let mut params = HashMap::new();
        params.insert("test_path".to_string(), "tests/unit/".to_string());
        let cmd = plugin.render_executor_command(&params).unwrap();
        assert_eq!(cmd, vec!["pytest", "tests/unit/"]);
    }

    #[test]
    fn render_uses_default_when_param_not_provided() {
        let plugin = make_executor_plugin(
            &["make", "{target}"],
            r#"
[[executor.params]]
name = "target"
default = "build"
"#,
        );
        let cmd = plugin.render_executor_command(&HashMap::new()).unwrap();
        assert_eq!(cmd, vec!["make", "build"]);
    }

    #[test]
    fn render_empty_token_dropped() {
        let plugin = make_executor_plugin(
            &["cargo", "test", "{args}"],
            r#"
[[executor.params]]
name = "args"
default = ""
"#,
        );
        let cmd = plugin.render_executor_command(&HashMap::new()).unwrap();
        assert_eq!(cmd, vec!["cargo", "test"]);
    }

    #[test]
    fn render_required_param_missing_errors() {
        let plugin = make_executor_plugin(
            &["cargo", "{subcommand}"],
            r#"
[[executor.params]]
name = "subcommand"
required = true
"#,
        );
        let err = plugin.render_executor_command(&HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("required param"));
        assert!(err.to_string().contains("subcommand"));
    }

    #[test]
    fn render_error_when_no_executor_spec() {
        let plugin: Plugin = toml::from_str(
            r#"
name = "plain"
description = "no executor"
category = "test"
[check]
binary = "plain"
"#,
        )
        .unwrap();
        let err = plugin.render_executor_command(&HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("no [executor] spec"));
    }

    #[test]
    fn markitdown_toml_parses() {
        let toml = r#"
name = "markitdown"
description = "Convert PDFs, Office files, images, and URLs to Markdown"
category = "tools"
url = "https://github.com/microsoft/markitdown"
use_orbit_venv = true

[check]
binary = "markitdown"

[[install]]
method = "pip"
cmd = ["pip", "install", "markitdown[mcp]"]
label = "orbit venv (pip)"

[[mcp]]
name = "markitdown"
command = "markitdown-mcp"
args = []
label = "MarkItDown MCP"
"#;
        let p: Plugin = toml::from_str(toml).expect("markitdown TOML should parse");
        assert_eq!(p.name, "markitdown");
        assert!(p.use_orbit_venv);
        assert_eq!(p.mcp.len(), 1);
    }
}
