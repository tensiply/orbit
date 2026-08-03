use anyhow::{Context, Result, bail};
use crate::output::truncate_desc;
use clap::{Args, Subcommand, ValueEnum};
use orbit_core::{catalog, catalog::McpEntry, context::OrbitScope};
use orbit_engine::resolver;
use serde_json::Value;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: Option<McpCommand>,

    /// Target scope level (default: deepest scope detected from cwd)
    #[arg(long, value_enum, global = true)]
    pub scope: Option<ScopeLevel>,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    /// List available MCPs and their status in the current scope
    List,
    /// Enable an MCP and write its config to the detected scope
    Enable {
        /// MCP name (from catalog)
        name: String,
    },
    /// Disable an MCP from the detected scope
    Disable {
        /// MCP name
        name: String,
    },
    /// Show MCP description, variables, and current status
    Info {
        /// MCP name
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScopeLevel {
    /// Global — available in every session (~/.config/orbit/mcps.json)
    Global,
    /// Tenant-level — available for the current tenant
    Tenant,
    /// Project-level — available for the current project
    Project,
    /// Repository-level — available for the current repository
    Repo,
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run(args: McpArgs) -> Result<()> {
    match args.command.unwrap_or(McpCommand::List) {
        McpCommand::List => cmd_list(args.scope),
        McpCommand::Enable { name } => cmd_enable(&name, args.scope),
        McpCommand::Disable { name } => cmd_disable(&name, args.scope),
        McpCommand::Info { name } => cmd_info(&name, args.scope),
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

fn cmd_list(scope_override: Option<ScopeLevel>) -> Result<()> {
    let scope = detect_scope_required(scope_override)?;
    let catalog_mcps = catalog::mcps();
    let scope_mcps = collect_all_scope_mcps(&scope, scope_override);
    let catalog_names: std::collections::HashSet<&str> =
        catalog_mcps.iter().map(|m| m.name.as_str()).collect();

    struct Row {
        name: String,
        desc: String,
        enabled_at: Option<String>,
        has_vars: bool,
    }

    let mut rows: Vec<Row> = Vec::new();

    for m in &catalog_mcps {
        let enabled_at = find_enabled_scope(&m.name, &scope, scope_override);
        rows.push(Row {
            name: m.name.clone(),
            desc: m.description.clone(),
            enabled_at,
            has_vars: !m.required_vars.is_empty(),
        });
    }

    // Custom MCPs: in scope files but not in catalog — always enabled
    let mut custom_names: Vec<String> = scope_mcps
        .keys()
        .filter(|n| !catalog_names.contains(n.as_str()))
        .cloned()
        .collect();
    custom_names.sort();
    for name in &custom_names {
        rows.push(Row {
            name: name.clone(),
            desc: String::new(),
            enabled_at: Some(scope_mcps[name].clone()),
            has_vars: false,
        });
    }

    // Stable sort: enabled first, disabled last; order within each group preserved
    rows.sort_by_key(|r| if r.enabled_at.is_some() { 0u8 } else { 1u8 });

    let total = rows.len();
    if total == 0 {
        println!("No MCPs available.");
        return Ok(());
    }

    let scope_label = scope_description(&scope, scope_override);
    println!("mcps\n");
    println!("  \x1b[2mMCPs extend orbit sessions with external tools and data sources.  (scope: {scope_label})\x1b[0m\n");

    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(8).max(8);
    let desc_w: usize = 48;
    let sep_w = 5 + name_w + 2 + desc_w + 2 + 20;

    println!(
        "     \x1b[2m{name:<name_w$}  {desc:<desc_w$}  status\x1b[0m",
        name = "name",
        desc = "description",
        name_w = name_w,
        desc_w = desc_w,
    );
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));

    for r in &rows {
        let (status, tag) = match &r.enabled_at {
            Some(lvl) => ("\x1b[32m●\x1b[0m", format!("\x1b[32m[{lvl}]\x1b[0m")),
            None => ("\x1b[2m○\x1b[0m", "\x1b[2m[disabled]\x1b[0m".to_string()),
        };
        let desc_raw = if r.desc.is_empty() {
            "(custom)".to_string()
        } else {
            r.desc.clone()
        };
        let desc = truncate_desc(&desc_raw, desc_w);
        let vars_tag = if r.has_vars {
            "  \x1b[33m⚙\x1b[0m"
        } else {
            ""
        };
        println!(
            "  {status}  {name:<name_w$}  {desc:<desc_w$}  {tag}{vars_tag}",
            name = r.name,
            name_w = name_w,
            desc_w = desc_w,
        );
    }

    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep_w));
    println!("  \x1b[2m● enabled  ○ disabled  ·  ⚙ requires variables\x1b[0m");

    println!();
    let enabled = rows.iter().filter(|r| r.enabled_at.is_some()).count();
    println!("  {enabled}/{total} enabled  ·  orbit mcp enable/disable <name>");

    Ok(())
}

// ── enable ────────────────────────────────────────────────────────────────────

fn cmd_enable(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    let entry = catalog::mcp_by_name(name).with_context(|| {
        format!("MCP not found in catalog: {name}\nRun `orbit mcp list` to see available MCPs.")
    })?;

    let (scope, level) = resolve_write_scope(scope_override)?;
    let path = mcp_json_path(scope.as_ref(), level);

    if mcp_in_file(name, &path) {
        println!("  \x1b[32m●\x1b[0m  {name} is already enabled at {level}.");
        return Ok(());
    }

    println!();
    println!("  {name}  —  {}", entry.description);
    println!();

    let env = collect_vars(&entry)?;
    let server = build_server_entry(&entry.command, &env);
    write_mcp_entry(&path, name, server)?;

    println!();
    println!("  \x1b[32m●\x1b[0m  {name} enabled at {level}");
    if !env.is_empty() {
        println!("     Config: {}", path.display());
    }
    println!("     Active in new orbit sessions.");

    Ok(())
}

// ── disable ───────────────────────────────────────────────────────────────────

fn cmd_disable(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    if catalog::mcp_by_name(name).is_none() {
        bail!("MCP not found in catalog: {name}\nRun `orbit mcp list` to see available MCPs.");
    }

    let (scope, level) = resolve_write_scope(scope_override)?;
    let path = mcp_json_path(scope.as_ref(), level);

    if !mcp_in_file(name, &path) {
        println!("  {name} is not enabled at {level}.");
        return Ok(());
    }

    remove_mcp_entry(&path, name)?;
    println!("  \x1b[32m✓\x1b[0m  {name} disabled at {level}");

    Ok(())
}

// ── info ──────────────────────────────────────────────────────────────────────

fn cmd_info(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    let entry = catalog::mcp_by_name(name).with_context(|| {
        format!("MCP not found in catalog: {name}\nRun `orbit mcp list` to see available MCPs.")
    })?;

    let scope = detect_scope_required(scope_override)?;

    let enabled_at = find_enabled_scope(name, &scope, scope_override);
    let status_str = match &enabled_at {
        Some(lvl) => format!("\x1b[32m● enabled at {lvl}\x1b[0m"),
        None => "\x1b[2m○ disabled\x1b[0m".to_string(),
    };

    println!();
    println!("  \x1b[1m{name}\x1b[0m");
    println!();
    println!("  description   {}", entry.description);
    println!("  command       {}", entry.command.join(" "));
    println!("  status        {status_str}");

    if !entry.required_vars.is_empty() {
        println!();
        println!("  required variables");
        for v in &entry.required_vars {
            let secret_tag = if v.secret {
                "  \x1b[33m[secret]\x1b[0m"
            } else {
                ""
            };
            println!("    {}{secret_tag}", v.name);
            println!("      \x1b[2m{}\x1b[0m", v.description);
        }
    }

    if !entry.optional_vars.is_empty() {
        println!();
        println!("  optional variables");
        for v in &entry.optional_vars {
            let default_tag = v
                .default
                .as_deref()
                .map(|d| format!("  \x1b[2m(default: {d})\x1b[0m"))
                .unwrap_or_default();
            println!("    {}{default_tag}", v.name);
            println!("      \x1b[2m{}\x1b[0m", v.description);
        }
    }

    // Status per scope layer
    println!();
    println!("  status by scope");
    let global_path = global_config_dir().join("orbit/mcps.json");
    let marker = if mcp_in_file(name, &global_path) {
        "\x1b[32m● enabled\x1b[0m "
    } else {
        "\x1b[2m○ disabled\x1b[0m"
    };
    println!("    global      {marker}");

    if !scope.global_mode {
        if !scope.tenant.is_empty() {
            let p = scope
                .ai_context_root
                .join("tenants")
                .join(&scope.tenant)
                .join("mcp.json");
            let marker = if mcp_in_file(name, &p) {
                "\x1b[32m● enabled\x1b[0m "
            } else {
                "\x1b[2m○ disabled\x1b[0m"
            };
            println!("    tenant      {marker}  \x1b[2m({})\x1b[0m", scope.tenant);
        }
        if !scope.project.is_empty() {
            let p = scope
                .ai_context_root
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project)
                .join("mcp.json");
            let marker = if mcp_in_file(name, &p) {
                "\x1b[32m● enabled\x1b[0m "
            } else {
                "\x1b[2m○ disabled\x1b[0m"
            };
            println!("    project     {marker}  \x1b[2m({})\x1b[0m", scope.project);
        }
        if !scope.repository.is_empty() {
            let p = scope
                .ai_context_root
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project)
                .join("repositories")
                .join(&scope.repository)
                .join("mcp.json");
            let marker = if mcp_in_file(name, &p) {
                "\x1b[32m● enabled\x1b[0m "
            } else {
                "\x1b[2m○ disabled\x1b[0m"
            };
            println!(
                "    repo        {marker}  \x1b[2m({})\x1b[0m",
                scope.repository
            );
        }
    }

    println!();
    Ok(())
}

// ── scope helpers ─────────────────────────────────────────────────────────────

/// Detect scope from cwd. Returns error if detection fails (for list/info).
/// For global scope override, returns a default OrbitScope (paths not used for listing).
fn detect_scope_required(scope_override: Option<ScopeLevel>) -> Result<OrbitScope> {
    if matches!(scope_override, Some(ScopeLevel::Global)) {
        return Ok(OrbitScope {
            global_mode: true,
            ..Default::default()
        });
    }
    resolver::resolve_from_cwd()
        .context("could not detect scope from current directory\nRun from inside a workspace, or use --scope global")
}

/// Returns (scope, level) for write operations.
/// For global scope, scope fields are unused (only global_config_dir is needed).
pub fn resolve_write_scope(
    scope_override: Option<ScopeLevel>,
) -> Result<(Option<OrbitScope>, ScopeLevel)> {
    match scope_override {
        Some(ScopeLevel::Global) => Ok((None, ScopeLevel::Global)),
        other => {
            let scope = resolver::resolve_from_cwd()
                .context("could not detect scope from current directory\nRun from inside a workspace, or use --scope global")?;
            let level = other.unwrap_or_else(|| default_level(&scope));
            validate_level(&scope, level)?;
            Ok((Some(scope), level))
        }
    }
}

fn default_level(scope: &OrbitScope) -> ScopeLevel {
    if !scope.repository.is_empty() {
        ScopeLevel::Repo
    } else if !scope.project.is_empty() {
        ScopeLevel::Project
    } else if !scope.tenant.is_empty() {
        ScopeLevel::Tenant
    } else {
        ScopeLevel::Global
    }
}

fn validate_level(scope: &OrbitScope, level: ScopeLevel) -> Result<()> {
    match level {
        ScopeLevel::Tenant if scope.tenant.is_empty() => {
            bail!("no tenant detected in current scope — cd into a tenant directory first")
        }
        ScopeLevel::Project if scope.project.is_empty() => {
            bail!("no project detected in current scope — cd into a project directory first")
        }
        ScopeLevel::Repo if scope.repository.is_empty() => {
            bail!("no repository detected in current scope — cd into a repository directory first")
        }
        _ => Ok(()),
    }
}

pub fn mcp_json_path(scope: Option<&OrbitScope>, level: ScopeLevel) -> PathBuf {
    match level {
        ScopeLevel::Global => global_config_dir().join("orbit/mcps.json"),
        ScopeLevel::Tenant => scope
            .unwrap()
            .ai_context_root
            .join("tenants")
            .join(&scope.unwrap().tenant)
            .join("mcp.json"),
        ScopeLevel::Project => scope
            .unwrap()
            .ai_context_root
            .join("tenants")
            .join(&scope.unwrap().tenant)
            .join("projects")
            .join(&scope.unwrap().project)
            .join("mcp.json"),
        ScopeLevel::Repo => scope
            .unwrap()
            .ai_context_root
            .join("tenants")
            .join(&scope.unwrap().tenant)
            .join("projects")
            .join(&scope.unwrap().project)
            .join("repositories")
            .join(&scope.unwrap().repository)
            .join("mcp.json"),
    }
}

fn scope_description(scope: &OrbitScope, override_level: Option<ScopeLevel>) -> String {
    if matches!(override_level, Some(ScopeLevel::Global)) || scope.global_mode {
        return "global".to_string();
    }
    let mut parts = Vec::new();
    if !scope.tenant.is_empty() {
        parts.push(scope.tenant.clone());
    }
    if !scope.project.is_empty() {
        parts.push(scope.project.clone());
    }
    if !scope.repository.is_empty() {
        parts.push(scope.repository.clone());
    }
    if parts.is_empty() {
        "workspace".to_string()
    } else {
        parts.join("/")
    }
}

fn find_enabled_scope(
    name: &str,
    scope: &OrbitScope,
    override_level: Option<ScopeLevel>,
) -> Option<String> {
    // Global
    let global_path = global_config_dir().join("orbit/mcps.json");
    if mcp_in_file(name, &global_path) {
        return Some("global".to_string());
    }
    if matches!(override_level, Some(ScopeLevel::Global)) || scope.global_mode {
        return None;
    }
    // Workspace (global_ai_root first, then ai_context_root if different)
    if mcp_in_file(name, &scope.global_ai_root.join("mcp.json")) {
        return Some("workspace".to_string());
    }
    if scope.ai_context_root != scope.global_ai_root
        && mcp_in_file(name, &scope.ai_context_root.join("mcp.json"))
    {
        return Some("workspace".to_string());
    }
    // Tenant
    if !scope.tenant.is_empty() {
        let p = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("mcp.json");
        if mcp_in_file(name, &p) {
            return Some(format!("tenant:{}", scope.tenant));
        }
    }
    // Project
    if !scope.project.is_empty() {
        let p = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("mcp.json");
        if mcp_in_file(name, &p) {
            return Some(format!("project:{}", scope.project));
        }
    }
    // Repo
    if !scope.repository.is_empty() {
        let p = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("repositories")
            .join(&scope.repository)
            .join("mcp.json");
        if mcp_in_file(name, &p) {
            return Some(format!("repo:{}", scope.repository));
        }
    }
    None
}

/// Returns all MCP names defined across all scope layers.
/// Higher-specificity layers overwrite the label from lower ones.
fn collect_all_scope_mcps(
    scope: &OrbitScope,
    override_level: Option<ScopeLevel>,
) -> std::collections::HashMap<String, String> {
    let mut found: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // Global
    for name in mcp_names_in_file(&global_config_dir().join("orbit/mcps.json")) {
        found.insert(name, "global".to_string());
    }
    if matches!(override_level, Some(ScopeLevel::Global)) || scope.global_mode {
        return found;
    }
    // Workspace
    for name in mcp_names_in_file(&scope.global_ai_root.join("mcp.json")) {
        found.insert(name, "workspace".to_string());
    }
    if scope.ai_context_root != scope.global_ai_root {
        for name in mcp_names_in_file(&scope.ai_context_root.join("mcp.json")) {
            found.insert(name, "workspace".to_string());
        }
    }
    // Tenant
    if !scope.tenant.is_empty() {
        let p = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("mcp.json");
        for name in mcp_names_in_file(&p) {
            found.insert(name, format!("tenant:{}", scope.tenant));
        }
    }
    // Project
    if !scope.project.is_empty() {
        let p = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("mcp.json");
        for name in mcp_names_in_file(&p) {
            found.insert(name, format!("project:{}", scope.project));
        }
    }
    // Repo
    if !scope.repository.is_empty() {
        let p = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("repositories")
            .join(&scope.repository)
            .join("mcp.json");
        for name in mcp_names_in_file(&p) {
            found.insert(name, format!("repo:{}", scope.repository));
        }
    }
    found
}

fn mcp_names_in_file(path: &Path) -> Vec<String> {
    if !path.is_file() {
        return vec![];
    }
    let Ok(text) = fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };
    val.get("mcpServers")
        .and_then(|s| s.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

// ── var prompts ───────────────────────────────────────────────────────────────

fn collect_vars(entry: &McpEntry) -> Result<std::collections::HashMap<String, String>> {
    let mut env = std::collections::HashMap::new();

    if !entry.required_vars.is_empty() {
        println!("Required variables:");
        for v in &entry.required_vars {
            let value = if v.secret {
                println!(
                    "  {} — {} \x1b[33m[secret: consider using an env var]\x1b[0m",
                    v.name, v.description
                );
                prompt_required(&v.name)?
            } else {
                println!("  {} — {}", v.name, v.description);
                prompt_required(&v.name)?
            };
            env.insert(v.name.clone(), value);
        }
    }

    if !entry.optional_vars.is_empty() {
        println!("Optional variables (press Enter to skip):");
        for v in &entry.optional_vars {
            let default_display = v
                .default
                .as_deref()
                .map(|d| format!(" [default: {d}]"))
                .unwrap_or_default();
            println!("  {} — {}{}", v.name, v.description, default_display);
            let value = prompt_optional(&v.name)?;
            if let Some(val) = value {
                env.insert(v.name.clone(), val);
            } else if let Some(def) = &v.default {
                env.insert(v.name.clone(), def.clone());
            }
        }
    }

    Ok(env)
}

fn prompt_required(name: &str) -> Result<String> {
    loop {
        print!("  {name}: ");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
        eprintln!("  {name} is required.");
    }
}

fn prompt_optional(name: &str) -> Result<Option<String>> {
    print!("  {name}: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let trimmed = line.trim().to_string();
    Ok(if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    })
}

// ── mcp.json read / write ─────────────────────────────────────────────────────

fn build_server_entry(
    command: &[String],
    env: &std::collections::HashMap<String, String>,
) -> Value {
    let (cmd, args) = command
        .split_first()
        .map(|(c, a)| (c.as_str(), a))
        .unwrap_or(("", &[]));
    let mut obj = serde_json::json!({
        "command": cmd,
        "args": args,
    });
    if !env.is_empty() {
        obj["env"] = serde_json::to_value(env).unwrap_or_default();
    }
    obj
}

pub fn write_mcp_entry(path: &Path, name: &str, server: Value) -> Result<()> {
    let mut val = read_mcp_file(path);
    val["mcpServers"][name] = server;
    write_mcp_file(path, &val)
}

pub fn remove_mcp_entry(path: &Path, name: &str) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let mut val = read_mcp_file(path);
    if let Some(servers) = val["mcpServers"].as_object_mut() {
        servers.remove(name);
    }
    write_mcp_file(path, &val)
}

fn read_mcp_file(path: &Path) -> Value {
    if path.is_file() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(empty_mcp_json)
    } else {
        empty_mcp_json()
    }
}

fn write_mcp_file(path: &Path, val: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(val)?)?;
    Ok(())
}

fn empty_mcp_json() -> Value {
    serde_json::json!({ "mcpServers": {} })
}

fn mcp_in_file(name: &str, path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    val.get("mcpServers")
        .and_then(|s| s.as_object())
        .is_some_and(|m| m.contains_key(name))
}

// ── misc helpers ──────────────────────────────────────────────────────────────

fn global_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".config"))
            .unwrap_or_else(|| PathBuf::from("/"))
    }
}

impl std::fmt::Display for ScopeLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeLevel::Global => write!(f, "global"),
            ScopeLevel::Tenant => write!(f, "tenant"),
            ScopeLevel::Project => write!(f, "project"),
            ScopeLevel::Repo => write!(f, "repo"),
        }
    }
}
