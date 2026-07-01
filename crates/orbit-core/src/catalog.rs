use serde::Deserialize;

const ENGINES_TOML: &str = include_str!("../../../config/catalog/engines.toml");
const MCPS_TOML: &str = include_str!("../../../config/catalog/mcps.toml");

// ── Engine catalog ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct EngineCatalogFile {
    engines: Vec<EngineEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EngineEntry {
    pub name: String,
    pub bin: String,
    /// npm package name. Empty for engines installed via other means.
    #[serde(default)]
    pub npm_package: String,
    /// Override install command for non-npm engines (e.g. gh extension install).
    /// When non-empty, used instead of `npm install -g <npm_package>`.
    #[serde(default)]
    pub install_cmd: Vec<String>,
    /// Override update command for non-npm engines.
    /// When non-empty, used instead of `npm install -g <npm_package>@latest`.
    #[serde(default)]
    pub update_cmd: Vec<String>,
    pub description: String,
    pub auth_hint: String,
    pub auth_cmd: String,
    #[serde(default)]
    pub auth_env_vars: Vec<String>,
    #[serde(default)]
    pub auth_config_dirs: Vec<String>,
}

// ── MCP catalog ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct McpCatalogFile {
    mcps: Vec<McpEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpEntry {
    pub name: String,
    pub description: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub required_vars: Vec<CatalogVar>,
    #[serde(default)]
    pub optional_vars: Vec<CatalogVar>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogVar {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub secret: bool,
    pub default: Option<String>,
}

// ── accessors ─────────────────────────────────────────────────────────────────

pub fn engines() -> Vec<EngineEntry> {
    toml::from_str::<EngineCatalogFile>(ENGINES_TOML)
        .expect("invalid engines catalog — this is a bug")
        .engines
}

pub fn mcps() -> Vec<McpEntry> {
    toml::from_str::<McpCatalogFile>(MCPS_TOML)
        .expect("invalid mcps catalog — this is a bug")
        .mcps
}

pub fn engine_by_name(name: &str) -> Option<EngineEntry> {
    engines().into_iter().find(|e| e.name == name)
}

pub fn mcp_by_name(name: &str) -> Option<McpEntry> {
    mcps().into_iter().find(|m| m.name == name)
}
