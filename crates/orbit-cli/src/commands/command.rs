use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use orbit_engine::{config::jsonc, resolver};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct CommandArgs {
    #[command(subcommand)]
    pub command: Option<CommandCommand>,

    /// Target scope level (default: deepest scope detected from cwd)
    #[arg(long, value_enum, global = true)]
    pub scope: Option<ScopeLevel>,
}

#[derive(Debug, Subcommand)]
pub enum CommandCommand {
    /// List all available commands with their enabled/disabled status for the current scope
    List,
    /// Show the description and body of a command
    Info {
        /// Command name
        name: String,
    },
    /// Enable a command for the target scope (adds it to the scope's commands list in orbit.json)
    Add {
        /// Command name (from `orbit command list`)
        name: String,
    },
    /// Enable a command for the target scope
    Enable {
        /// Command name
        name: String,
    },
    /// Disable a command for the target scope (removes it from the scope's commands list)
    Disable {
        /// Command name
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScopeLevel {
    /// Workspace root — {ai_context_root}/orbit.json
    Workspace,
    /// Tenant level — tenants/{tenant}/orbit.json
    Tenant,
    /// Project level — tenants/{tenant}/projects/{project}/orbit.json
    Project,
    /// Repository level — …/repositories/{repo}/orbit.json
    Repo,
}

impl std::fmt::Display for ScopeLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeLevel::Workspace => write!(f, "workspace"),
            ScopeLevel::Tenant => write!(f, "tenant"),
            ScopeLevel::Project => write!(f, "project"),
            ScopeLevel::Repo => write!(f, "repo"),
        }
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run(args: CommandArgs) -> Result<()> {
    match args.command.unwrap_or(CommandCommand::List) {
        CommandCommand::List => cmd_list(args.scope),
        CommandCommand::Info { name } => cmd_info(&name),
        CommandCommand::Add { name } | CommandCommand::Enable { name } => {
            cmd_enable(&name, args.scope)
        }
        CommandCommand::Disable { name } => cmd_disable(&name, args.scope),
    }
}

// ── handlers ──────────────────────────────────────────────────────────────────

fn cmd_list(scope_override: Option<ScopeLevel>) -> Result<()> {
    let scope = detect_scope()?;
    let shared = scope.global_ai_root.join("source-of-truth/orbit");
    let local = scope.ai_context_root.join("source-of-truth/orbit");

    let catalog = discover_catalog(&shared, &local);
    if catalog.is_empty() {
        println!("no commands found in catalog");
        return Ok(());
    }

    // Compute the effective enabled set for the current scope (union across all layers)
    let enabled = effective_enabled_set(&scope, scope_override);

    println!("commands\n");
    let name_w = catalog.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    for (name, desc) in &catalog {
        let pad = " ".repeat(name_w.saturating_sub(name.len()));
        let (marker, label) = match &enabled {
            None => (
                "\x1b[32m●\x1b[0m",
                "\x1b[2m(all enabled — no filter set)\x1b[0m",
            ),
            Some(set) if set.contains(name.as_str()) => ("\x1b[32m●\x1b[0m", ""),
            Some(_) => ("\x1b[2m○\x1b[0m", "\x1b[2mdisabled\x1b[0m"),
        };
        let desc_str = desc.as_deref().unwrap_or("");
        println!("  {marker}  {name}{pad}  {desc_str}  {label}");
    }
    println!();

    match &enabled {
        None => {
            println!("  \x1b[2mNo commands filter set — all commands are enabled.\x1b[0m");
            println!(
                "  \x1b[2mRun `orbit command add <name>` to start managing commands for this scope.\x1b[0m"
            );
        }
        Some(set) => {
            let level = scope_override
                .unwrap_or_else(|| default_level(&scope))
                .to_string();
            println!(
                "  \x1b[2m{} command(s) enabled at {level} scope.\x1b[0m",
                set.len()
            );
        }
    }
    Ok(())
}

fn cmd_info(name: &str) -> Result<()> {
    let scope = detect_scope()?;
    let shared = scope.global_ai_root.join("source-of-truth/orbit");
    let local = scope.ai_context_root.join("source-of-truth/orbit");

    let candidates = [
        shared.join("commands").join(format!("{name}.md")),
        local.join("commands").join(format!("{name}.md")),
    ];
    let content = candidates
        .iter()
        .find_map(|p| fs::read_to_string(p).ok())
        .ok_or_else(|| anyhow::anyhow!("command '{name}' not found in catalog"))?;

    println!("command: {name}\n");
    println!("{}", content.trim());
    Ok(())
}

fn cmd_enable(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    // Validate that the command exists in the catalog
    let scope = detect_scope()?;
    let shared = scope.global_ai_root.join("source-of-truth/orbit");
    let local = scope.ai_context_root.join("source-of-truth/orbit");
    let catalog = discover_catalog(&shared, &local);

    if !catalog.iter().any(|(n, _)| n == name) {
        bail!(
            "command '{name}' not found in catalog\n\
             Run `orbit command list` to see available commands."
        );
    }

    let level = scope_override.unwrap_or_else(|| default_level(&scope));
    validate_level(&scope, level)?;
    let path = orbit_json_path(&scope, level);

    let mut val = read_orbit_json(&path);
    let commands = val["commands"].as_array_mut().cloned().unwrap_or_default();

    let mut names: Vec<String> = commands
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();

    if names.contains(&name.to_string()) {
        println!("  \x1b[33m~\x1b[0m  '{name}' already enabled at {level}");
        return Ok(());
    }

    names.push(name.to_string());
    names.sort();
    val["commands"] = Value::Array(names.into_iter().map(Value::String).collect());
    write_orbit_json(&path, &val)?;

    println!("  \x1b[32m✓\x1b[0m  '{name}' enabled at {level}");
    println!("       written to: {}", path.display());
    Ok(())
}

fn cmd_disable(name: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    let scope = detect_scope()?;
    let level = scope_override.unwrap_or_else(|| default_level(&scope));
    validate_level(&scope, level)?;
    let path = orbit_json_path(&scope, level);

    if !path.is_file() {
        bail!("no orbit.json at {level} scope ({})", path.display());
    }

    let mut val = read_orbit_json(&path);
    let commands = val["commands"].as_array().cloned().unwrap_or_default();
    let names: Vec<Value> = commands
        .into_iter()
        .filter(|v| v.as_str() != Some(name))
        .collect();

    if names.len() == val["commands"].as_array().map(|a| a.len()).unwrap_or(0) {
        bail!("'{name}' is not in the {level} commands list");
    }

    if names.is_empty() {
        val.as_object_mut().unwrap().remove("commands");
        println!(
            "  \x1b[32m✓\x1b[0m  '{name}' disabled at {level} (commands list removed — all commands now enabled)"
        );
    } else {
        val["commands"] = Value::Array(names);
        println!("  \x1b[32m✓\x1b[0m  '{name}' disabled at {level}");
    }

    write_orbit_json(&path, &val)?;
    println!("       written to: {}", path.display());
    Ok(())
}

// ── catalog helpers ───────────────────────────────────────────────────────────

/// Discover the full command catalog:
/// 1. Built-in commands embedded in binary (lowest priority — overridable)
/// 2. Source-of-truth commands from shared/local dirs (override built-ins by name)
/// 3. User commands from `~/.config/orbit/commands/`
/// Returns (name, description) pairs, deduped (later entries win for description).
fn discover_catalog(shared: &Path, local: &Path) -> Vec<(String, Option<String>)> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    // 1. Built-ins
    for (name, content) in orbit_core::builtin_command::all() {
        seen.insert(name.to_string());
        result.push((name.to_string(), extract_description_from_str(content)));
    }

    // 2. Source-of-truth commands (may override built-in description)
    for dir in [shared.join("commands"), local.join("commands")] {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let desc = extract_description(&path);
            if seen.contains(stem) {
                // Update description if the source-of-truth overrides it
                if let Some(entry) = result.iter_mut().find(|(n, _)| n == stem) {
                    entry.1 = desc;
                }
            } else {
                seen.insert(stem.to_string());
                result.push((stem.to_string(), desc));
            }
        }
    }

    // 3. User commands from ~/.config/orbit/commands/
    let user_dir = user_commands_dir();
    if let Ok(entries) = fs::read_dir(&user_dir) {
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if seen.contains(stem) {
                continue;
            }
            seen.insert(stem.to_string());
            result.push((stem.to_string(), extract_description(&path)));
        }
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn user_commands_dir() -> PathBuf {
    std::env::var("ORBIT_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            directories::BaseDirs::new()
                .map(|b| b.home_dir().join(".config"))
                .unwrap_or_default()
        })
        .join("orbit/commands")
}

/// Extract `description:` from a command file on disk.
fn extract_description(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    extract_description_from_str(&text)
}

/// Extract `description:` from a string (used for built-in content).
fn extract_description_from_str(content: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }
    for line in content.lines().skip(1) {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("description:") {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

// ── enabled-set helpers ───────────────────────────────────────────────────────

/// Build the union of all `commands` arrays across all scope layers.
/// Returns `None` if no layer specifies commands (= all enabled).
fn effective_enabled_set(
    scope: &orbit_core::context::OrbitScope,
    _scope_override: Option<ScopeLevel>,
) -> Option<std::collections::HashSet<String>> {
    let layers = scope_layers(scope);
    let mut result: Option<std::collections::HashSet<String>> = None;

    for (_, path) in layers {
        if let Some(names) = read_commands_from_file(&path) {
            match &mut result {
                None => result = Some(names.into_iter().collect()),
                Some(set) => set.extend(names),
            }
        }
    }
    result
}

fn read_commands_from_file(path: &Path) -> Option<Vec<String>> {
    if !path.is_file() {
        return None;
    }
    let val = jsonc::load_file(path);
    let arr = val.get("commands")?.as_array()?;
    let names: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

// ── orbit.json helpers ────────────────────────────────────────────────────────

fn read_orbit_json(path: &Path) -> Value {
    if path.is_file() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    }
}

fn write_orbit_json(path: &Path, val: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(val)?)?;
    Ok(())
}

// ── scope helpers ─────────────────────────────────────────────────────────────

fn detect_scope() -> Result<orbit_core::context::OrbitScope> {
    resolver::resolve_from_cwd().map_err(|_| {
        anyhow::anyhow!(
            "could not detect scope from current directory\n\
             cd into a workspace or use --scope to specify the target level"
        )
    })
}

fn default_level(scope: &orbit_core::context::OrbitScope) -> ScopeLevel {
    if !scope.repository.is_empty() {
        ScopeLevel::Repo
    } else if !scope.project.is_empty() {
        ScopeLevel::Project
    } else if !scope.tenant.is_empty() {
        ScopeLevel::Tenant
    } else {
        ScopeLevel::Workspace
    }
}

fn validate_level(scope: &orbit_core::context::OrbitScope, level: ScopeLevel) -> Result<()> {
    match level {
        ScopeLevel::Tenant if scope.tenant.is_empty() => {
            bail!("no tenant detected — cd into a tenant directory first")
        }
        ScopeLevel::Project if scope.project.is_empty() => {
            bail!("no project detected — cd into a project directory first")
        }
        ScopeLevel::Repo if scope.repository.is_empty() => {
            bail!("no repository detected — cd into a repository directory first")
        }
        _ => Ok(()),
    }
}

fn orbit_json_path(scope: &orbit_core::context::OrbitScope, level: ScopeLevel) -> PathBuf {
    match level {
        ScopeLevel::Workspace => scope.ai_context_root.join("orbit.json"),
        ScopeLevel::Tenant => scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("orbit.json"),
        ScopeLevel::Project => scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("orbit.json"),
        ScopeLevel::Repo => scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("repositories")
            .join(&scope.repository)
            .join("orbit.json"),
    }
}

fn scope_layers(scope: &orbit_core::context::OrbitScope) -> Vec<(String, PathBuf)> {
    let mut layers = Vec::new();
    layers.push((
        "workspace".to_string(),
        scope.ai_context_root.join("orbit.json"),
    ));
    if !scope.global_mode {
        if !scope.tenant.is_empty() {
            layers.push((
                format!("tenant:{}", scope.tenant),
                scope
                    .ai_context_root
                    .join("tenants")
                    .join(&scope.tenant)
                    .join("orbit.json"),
            ));
        }
        if !scope.project.is_empty() {
            layers.push((
                format!("project:{}", scope.project),
                scope
                    .ai_context_root
                    .join("tenants")
                    .join(&scope.tenant)
                    .join("projects")
                    .join(&scope.project)
                    .join("orbit.json"),
            ));
        }
        if !scope.repository.is_empty() {
            layers.push((
                format!("repo:{}", scope.repository),
                scope
                    .ai_context_root
                    .join("tenants")
                    .join(&scope.tenant)
                    .join("projects")
                    .join(&scope.project)
                    .join("repositories")
                    .join(&scope.repository)
                    .join("orbit.json"),
            ));
        }
    }
    layers
}
