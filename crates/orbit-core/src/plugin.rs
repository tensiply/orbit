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
    /// at `~/.orbit/data/venv/` instead of the system Python environment.
    #[serde(default)]
    pub use_orbit_venv: bool,
    /// Dynamic multi-instance spec.  When present, `orbit plugins auth` enters
    /// the instance flow instead of the static var flow.
    pub instance: Option<InstanceSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckSpec {
    /// Primary binary — checked in the orbit venv when `use_orbit_venv = true`.
    pub binary: Option<String>,
    /// Alternative binaries checked in the system PATH (any match → installed).
    #[serde(default)]
    pub any_of: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallMethod {
    pub method: String,
    pub cmd: Vec<String>,
    pub label: String,
    /// Binary that must be present for this step to be considered installed.
    /// If set and the binary is missing, `orbit plugins install` will offer to run this step
    /// even when the plugin's primary check already passes.
    #[serde(default)]
    pub check: Option<String>,
}

impl InstallMethod {
    pub fn is_step_installed(&self) -> bool {
        match &self.check {
            Some(bin) => bin_available(bin),
            None => true, // no per-step check → assume present
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthSpec {
    pub hint: String,
    /// Shell command to run after collecting vars (e.g. "acli jira auth login")
    pub cmd: Option<String>,
    /// Credential vars to collect interactively and store in the OS keychain
    #[serde(default)]
    pub vars: Vec<AuthVar>,
    /// OAuth 2.1 + PKCE + dynamic client registration flow.
    /// When present, `orbit plugins auth` opens a browser instead of prompting for vars.
    pub oauth: Option<OAuthSpec>,
}

/// OAuth 2.1 PKCE flow with dynamic client registration (RFC 7591).
/// The MCP server at `discovery_url` exposes the standard
/// `/.well-known/oauth-authorization-server` metadata document.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthSpec {
    /// URL of the OAuth discovery document.
    pub discovery_url: String,
    /// Keychain key used to store the resulting access token.
    pub token_key: String,
    /// Space-separated OAuth scopes to request.
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthVar {
    /// Keychain key and env var name (e.g. "SONARCLOUD_TOKEN")
    pub name: String,
    /// Human-readable prompt shown to the user
    pub description: String,
    /// Mask input in the terminal — stored encrypted in keychain regardless
    #[serde(default)]
    pub secret: bool,
    /// Allow empty value (skip storing if blank)
    #[serde(default)]
    pub optional: bool,
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
    /// HTTP headers forwarded to remote MCP servers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

// ── dynamic multi-instance spec ───────────────────────────────────────────────

/// When present on a plugin, `orbit plugins auth` enters the multi-instance flow
/// instead of the static var flow.  Each configured instance generates one MCP
/// entry named `<plugin>-<instance>`.
#[derive(Debug, Clone, Deserialize)]
pub struct InstanceSpec {
    /// Vars collected interactively per instance (same structure as `AuthVar`).
    pub vars: Vec<AuthVar>,
    /// MCP entry template — `{var_name}` placeholders are substituted at generation time.
    pub mcp: InstanceMcpTemplate,
    /// Optional HTTP Basic auth derivation: combines two vars into a base64-encoded
    /// `user:pass` secret and exposes it as a derived var for use in MCP templates.
    #[serde(default)]
    pub basic_auth: Option<BasicAuthSpec>,
}

/// Derives a base64-encoded `user:pass` var from two collected instance vars.
#[derive(Debug, Clone, Deserialize)]
pub struct BasicAuthSpec {
    /// Name of the derived var injected into MCP template substitution (e.g. `"auth"`).
    pub var: String,
    /// Instance var holding the username.
    pub user: String,
    /// Instance var holding the password or API token.
    pub pass: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstanceMcpTemplate {
    // HTTP/SSE mode — mutually exclusive with command.
    /// URL template, e.g. `"{url}/mcp"`. When set, generates an HTTP MCP entry.
    pub url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    // stdio mode — used when `url` is absent.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Env vars injected into the MCP process. Values support `{var_name}` substitution.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

// ── instance state ────────────────────────────────────────────────────────────

/// Persisted at `~/.orbit/plugin-instances.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginInstances {
    #[serde(default)]
    pub instances: Vec<PluginInstanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInstanceRecord {
    pub plugin: String,
    pub name: String,
    /// Non-secret vars stored here; secrets live in the OS keychain.
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

impl PluginInstances {
    pub fn path() -> PathBuf {
        user_config_dir().join("plugin-instances.toml")
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
        fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn for_plugin<'a>(
        &'a self,
        plugin: &'a str,
    ) -> impl Iterator<Item = &'a PluginInstanceRecord> {
        self.instances.iter().filter(move |r| r.plugin == plugin)
    }

    pub fn upsert(&mut self, record: PluginInstanceRecord) {
        if let Some(existing) = self
            .instances
            .iter_mut()
            .find(|r| r.plugin == record.plugin && r.name == record.name)
        {
            *existing = record;
        } else {
            self.instances.push(record);
        }
    }

    pub fn remove(&mut self, plugin: &str, name: &str) -> bool {
        let before = self.instances.len();
        self.instances
            .retain(|r| !(r.plugin == plugin && r.name == name));
        self.instances.len() < before
    }
}

// ── plugin state ──────────────────────────────────────────────────────────────

/// Tracks which plugins are enabled (MCP servers active).
/// Persisted at `~/.orbit/plugin-state.toml`.
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
            let primary_ok = if self.use_orbit_venv {
                crate::venv::venv_bin(bin).exists()
            } else {
                bin_available(bin)
            };
            if primary_ok {
                return true;
            }
        }
        // Any alternative binary available in the system PATH counts as installed.
        self.check.any_of.iter().any(|b| bin_available(b))
    }

    pub fn has_mcp(&self) -> bool {
        !self.mcp.is_empty() || self.instance.is_some()
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

/// Load all plugins: built-ins first, then user plugins (`~/.orbit/plugins/`).
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

// ── MCP entry builder ─────────────────────────────────────────────────────────

/// Build all MCP entries for a plugin (static + instances) as `(name, JSON)` pairs.
/// Reads secrets from the OS keychain for instance vars. Does not write anywhere.
pub fn build_mcp_entries(
    plugin: &Plugin,
    instances: &PluginInstances,
) -> Result<Vec<(String, serde_json::Value)>> {
    let mut out = Vec::new();

    // Static MCPs
    for entry in &plugin.mcp {
        let server = if let Some(url) = &entry.url {
            let mut s = serde_json::json!({ "type": "http", "url": url });
            if !entry.headers.is_empty() {
                s["headers"] = serde_json::to_value(&entry.headers)?;
            }
            s
        } else {
            let command = if plugin.use_orbit_venv {
                crate::venv::venv_bin(&entry.command)
                    .to_string_lossy()
                    .to_string()
            } else {
                entry.command.clone()
            };
            let mut s = serde_json::json!({ "command": command, "args": entry.args });
            if !entry.env.is_empty() {
                s["env"] = serde_json::to_value(&entry.env)?;
            }
            s
        };
        out.push((entry.name.clone(), server));
    }

    // Instance MCPs
    if let Some(spec) = &plugin.instance {
        for record in instances.for_plugin(&plugin.name) {
            let mut all_vars: HashMap<String, String> = record.vars.clone();
            for var in &spec.vars {
                if var.secret {
                    let key = instance_keychain_key(&plugin.name, &record.name, &var.name);
                    if let Ok(secret) = crate::secrets::keychain_get(&key) {
                        all_vars.insert(var.name.clone(), secret);
                    }
                }
            }
            if let Some(ba) = &spec.basic_auth
                && let (Some(user), Some(pass)) = (all_vars.get(&ba.user), all_vars.get(&ba.pass))
            {
                use base64::Engine as _;
                let encoded = base64::engine::general_purpose::STANDARD
                    .encode(format!("{user}:{pass}"));
                all_vars.insert(ba.var.clone(), encoded);
            }
            let entry = if let Some(url_template) = &spec.mcp.url {
                let url = substitute(url_template, &all_vars);
                let mut e = serde_json::json!({ "type": "http", "url": url });
                if !spec.mcp.headers.is_empty() {
                    let headers: HashMap<String, String> = spec
                        .mcp
                        .headers
                        .iter()
                        .map(|(k, v)| (k.clone(), substitute(v, &all_vars)))
                        .collect();
                    e["headers"] = serde_json::to_value(&headers)?;
                }
                e
            } else {
                let mut e = serde_json::json!({
                    "command": substitute(&spec.mcp.command, &all_vars),
                    "args": spec.mcp.args.iter().map(|a| substitute(a, &all_vars)).collect::<Vec<_>>(),
                });
                if !spec.mcp.env.is_empty() {
                    let env: HashMap<String, String> = spec
                        .mcp
                        .env
                        .iter()
                        .filter_map(|(k, v)| {
                            let val = substitute(v, &all_vars);
                            // Drop empty values and unresolved placeholders.
                            if val.is_empty() || val.contains('{') { None } else { Some((k.clone(), val)) }
                        })
                        .collect();
                    if !env.is_empty() {
                        e["env"] = serde_json::to_value(&env)?;
                    }
                }
                e
            };
            out.push((format!("{}-{}", plugin.name, record.name), entry));
        }
    }

    Ok(out)
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
            let mut s = serde_json::json!({ "type": "http", "url": url });
            if !entry.headers.is_empty() {
                s["headers"] = serde_json::to_value(&entry.headers)?;
            }
            s
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

// ── instance MCP management ───────────────────────────────────────────────────

/// Keychain key for an instance secret: `PLUGIN_INSTANCE_VAR` (all uppercase, dashes → underscores).
pub fn instance_keychain_key(plugin: &str, instance_name: &str, var: &str) -> String {
    let normalize = |s: &str| s.to_uppercase().replace('-', "_");
    format!("{}_{}_{}",  normalize(plugin), normalize(instance_name), normalize(var))
}

fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("{{{k}}}"), v);
    }
    result
}

/// Write one MCP entry per configured instance of this plugin into `plugins.mcp.json`.
/// Entry name: `<plugin>-<instance>`.
pub fn add_instance_mcps(plugin: &Plugin, instances: &PluginInstances) -> Result<()> {
    let Some(spec) = &plugin.instance else {
        return Ok(());
    };
    let path = plugins_mcp_path();
    let mut val = read_plugins_mcp_file(&path);
    let servers = val["mcpServers"]
        .as_object_mut()
        .expect("mcpServers should be an object");

    for record in instances.for_plugin(&plugin.name) {
        let mut all_vars: HashMap<String, String> = record.vars.clone();
        for var in &spec.vars {
            if var.secret {
                let key = instance_keychain_key(&plugin.name, &record.name, &var.name);
                if let Ok(secret) = crate::secrets::keychain_get(&key) {
                    all_vars.insert(var.name.clone(), secret);
                }
            }
        }
        if let Some(ba) = &spec.basic_auth
            && let (Some(user), Some(pass)) = (all_vars.get(&ba.user), all_vars.get(&ba.pass))
        {
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{user}:{pass}"));
            all_vars.insert(ba.var.clone(), encoded);
        }
        let entry = if let Some(url_template) = &spec.mcp.url {
            let url = substitute(url_template, &all_vars);
            let mut e = serde_json::json!({ "type": "http", "url": url });
            if !spec.mcp.headers.is_empty() {
                let headers: HashMap<String, String> = spec
                    .mcp
                    .headers
                    .iter()
                    .map(|(k, v)| (k.clone(), substitute(v, &all_vars)))
                    .collect();
                e["headers"] = serde_json::to_value(&headers)?;
            }
            e
        } else {
            let mut e = serde_json::json!({
                "command": substitute(&spec.mcp.command, &all_vars),
                "args": spec.mcp.args.iter().map(|a| substitute(a, &all_vars)).collect::<Vec<_>>(),
            });
            if !spec.mcp.env.is_empty() {
                let env: HashMap<String, String> = spec
                    .mcp
                    .env
                    .iter()
                    .filter(|(_, v)| !substitute(v, &all_vars).is_empty())
                    .map(|(k, v)| (k.clone(), substitute(v, &all_vars)))
                    .collect();
                if !env.is_empty() {
                    e["env"] = serde_json::to_value(&env)?;
                }
            }
            e
        };
        servers.insert(format!("{}-{}", plugin.name, record.name), entry);
    }

    write_plugins_mcp_file(&path, &val)
}

/// Remove all instance MCP entries for this plugin from `plugins.mcp.json`.
pub fn remove_instance_mcps(plugin: &Plugin, instances: &PluginInstances) -> Result<()> {
    let path = plugins_mcp_path();
    if !path.is_file() {
        return Ok(());
    }
    let mut val = read_plugins_mcp_file(&path);
    if let Some(servers) = val["mcpServers"].as_object_mut() {
        for record in instances.for_plugin(&plugin.name) {
            servers.remove(&format!("{}-{}", plugin.name, record.name));
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
    crate::data_paths::orbit_home()
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
