use anyhow::{Context, Result, bail};
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
    let mcps = catalog::mcps();

    if mcps.is_empty() {
        println!("No MCPs in catalog.");
        return Ok(());
    }

    let scope_label = scope_description(&scope, scope_override);
    println!("mcps  \x1b[2m(scope: {scope_label})\x1b[0m\n");

    let name_w = mcps.iter().map(|m| m.name.len()).max().unwrap_or(8).max(8);

    for m in &mcps {
        let enabled_at = find_enabled_scope(&m.name, &scope, scope_override);
        let (status, tag) = match &enabled_at {
            Some(lvl) => ("\x1b[32m●\x1b[0m", format!("  \x1b[32m[{lvl}]\x1b[0m")),
            None => ("\x1b[2m○\x1b[0m", "\x1b[2m  [disabled]\x1b[0m".to_string()),
        };

        let vars_hint = if !m.required_vars.is_empty() {
            format!(
                "  \x1b[33m({} var{})\x1b[0m",
                m.required_vars.len(),
                if m.required_vars.len() == 1 { "" } else { "s" }
            )
        } else {
            String::new()
        };

        println!(
            "  {status}  {name:<name_w$}  {desc}{tag}{vars_hint}",
            name = m.name,
            desc = m.description,
            name_w = name_w,
        );
    }

    println!();
    let enabled = mcps
        .iter()
        .filter(|m| find_enabled_scope(&m.name, &scope, scope_override).is_some())
        .count();
    println!(
        "  {enabled}/{total} enabled  ·  orbit mcp enable/disable <name>",
        total = mcps.len()
    );

    Ok(())
}

// ── enable ────────────────────────────────────────────────────────────────────

fn cmd_enable(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    let entry = catalog::mcp_by_name(name).with_context(|| {
        format!("MCP not found in catalog: {name}\nRun `orbit mcp list` to see available MCPs.")
    })?;

    let (scope, level) = resolve_write_scope(scope_override)?;
    let path = mcp_json_path(scope.as_ref(), level);

    // Check already enabled
    if mcp_in_file(name, &path) {
        println!("  \x1b[32m●\x1b[0m  {name} is already enabled at {level}.");
        return Ok(());
    }

    println!("Enabling \x1b[1m{name}\x1b[0m at {level}");
    println!("  {}", entry.description);
    println!();

    // Collect vars
    let env = collect_vars(&entry)?;

    // Build server entry
    let server = build_server_entry(&entry.command, &env);

    // Write to mcp.json
    write_mcp_entry(&path, name, server)?;

    println!();
    println!("  \x1b[32m✓\x1b[0m  {name} enabled at {level}");
    if !env.is_empty() {
        println!("       Config written to: {}", path.display());
    }

    Ok(())
}

// ── disable ───────────────────────────────────────────────────────────────────

fn cmd_disable(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    // Verify it exists in catalog
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

    println!("\x1b[1m{name}\x1b[0m");
    println!("  {}", entry.description);
    println!();
    println!("  command: {}", entry.command.join(" "));
    println!();

    if !entry.required_vars.is_empty() {
        println!("  Required variables:");
        for v in &entry.required_vars {
            let secret_tag = if v.secret {
                "  \x1b[33m[secret]\x1b[0m"
            } else {
                ""
            };
            println!("    {}{secret_tag}", v.name);
            println!("      {}", v.description);
        }
        println!();
    }

    if !entry.optional_vars.is_empty() {
        println!("  Optional variables:");
        for v in &entry.optional_vars {
            let default_tag = v
                .default
                .as_deref()
                .map(|d| format!("  (default: {d})"))
                .unwrap_or_default();
            println!("    {}{default_tag}", v.name);
            println!("      {}", v.description);
        }
        println!();
    }

    // Status per scope layer
    println!("  Status:");
    let global_path = global_config_dir().join("orbit/mcps.json");
    let marker = if mcp_in_file(name, &global_path) {
        "\x1b[32m● enabled\x1b[0m"
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
                "\x1b[32m● enabled\x1b[0m"
            } else {
                "\x1b[2m○ disabled\x1b[0m"
            };
            println!("    tenant      {marker}  ({})", scope.tenant);
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
                "\x1b[32m● enabled\x1b[0m"
            } else {
                "\x1b[2m○ disabled\x1b[0m"
            };
            println!("    project     {marker}  ({})", scope.project);
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
                "\x1b[32m● enabled\x1b[0m"
            } else {
                "\x1b[2m○ disabled\x1b[0m"
            };
            println!("    repo        {marker}  ({})", scope.repository);
        }
    }

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
fn resolve_write_scope(
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

fn mcp_json_path(scope: Option<&OrbitScope>, level: ScopeLevel) -> PathBuf {
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

fn write_mcp_entry(path: &Path, name: &str, server: Value) -> Result<()> {
    let mut val = read_mcp_file(path);
    val["mcpServers"][name] = server;
    write_mcp_file(path, &val)
}

fn remove_mcp_entry(path: &Path, name: &str) -> Result<()> {
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
