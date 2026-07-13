use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use orbit_core::engine::Engine;
use orbit_core::{context::OrbitScope, secrets};
use orbit_engine::{config, resolver};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct EnvArgs {
    #[command(subcommand)]
    pub command: Option<EnvCommand>,

    /// Target scope level (default: deepest scope detected from cwd)
    #[arg(long, value_enum, global = true)]
    pub scope: Option<ScopeLevel>,
}

#[derive(Debug, Subcommand)]
pub enum EnvCommand {
    /// Set an env var in the target orbit.json
    Set {
        /// Variable name
        key: String,
        /// Value (use `keychain://KEY`, `file:///path`, `env://VAR`, or a literal)
        value: String,
    },
    /// Show the resolved value of an env var
    Get {
        /// Variable name
        key: String,
    },
    /// Remove an env var from the target orbit.json
    Delete {
        /// Variable name
        key: String,
    },
    /// List env vars across all scope layers
    List,
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

pub fn run(args: EnvArgs) -> Result<()> {
    match args.command.unwrap_or(EnvCommand::List) {
        EnvCommand::Set { key, value } => cmd_set(&key, &value, args.scope),
        EnvCommand::Get { key } => cmd_get(&key),
        EnvCommand::Delete { key } => cmd_delete(&key, args.scope),
        EnvCommand::List => cmd_list(args.scope),
    }
}

// ── handlers ──────────────────────────────────────────────────────────────────

fn cmd_set(key: &str, value: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    let (scope, level) = resolve_write_scope(scope_override)?;
    let path = orbit_json_path(scope.as_ref(), level);

    let mut val = read_orbit_json(&path);
    val["env"][key] = Value::String(value.to_string());
    write_orbit_json(&path, &val)?;

    println!("  \x1b[32m✓\x1b[0m  {key} = {value}");
    println!("       written to: {}", path.display());
    if value.starts_with("keychain://") {
        let k = value.trim_start_matches("keychain://");
        println!("       \x1b[2mhint: orbit secret set {k} <value>\x1b[0m");
    }
    Ok(())
}

fn cmd_get(key: &str) -> Result<()> {
    let scope = detect_scope()?;
    let engine = Engine::Claude; // engine doesn't affect env merge
    let merged = config::load(&scope, engine)?;

    match merged.env.get(key) {
        None => bail!("env var '{key}' not found in any scope layer"),
        Some(raw) => {
            let resolved = secrets::resolve(raw);
            if raw != &resolved {
                println!("{resolved}");
            } else {
                println!("{raw}");
            }
        }
    }
    Ok(())
}

fn cmd_delete(key: &str, scope_override: Option<ScopeLevel>) -> Result<()> {
    let (scope, level) = resolve_write_scope(scope_override)?;
    let path = orbit_json_path(scope.as_ref(), level);

    if !path.is_file() {
        bail!("no orbit.json at {level} scope ({})", path.display());
    }

    let mut val = read_orbit_json(&path);
    let removed = val["env"]
        .as_object_mut()
        .is_some_and(|m| m.remove(key).is_some());

    if !removed {
        bail!("'{key}' not found in {level} orbit.json");
    }

    // Clean up empty env block
    if val["env"].as_object().is_some_and(|m| m.is_empty()) {
        val.as_object_mut().unwrap().remove("env");
    }

    write_orbit_json(&path, &val)?;
    println!("  \x1b[32m✓\x1b[0m  {key} removed from {level}");
    Ok(())
}

fn cmd_list(scope_override: Option<ScopeLevel>) -> Result<()> {
    let scope = detect_scope_for_list(scope_override)?;
    let engine = Engine::Claude;
    let merged = config::load(&scope, engine)?;

    if merged.env.is_empty() {
        println!("no env vars configured for this scope");
        return Ok(());
    }

    println!("env vars\n");

    // Per-layer breakdown
    let layers = env_layers(&scope);
    let key_w = merged.env.keys().map(|k| k.len()).max().unwrap_or(0);

    let mut printed: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (label, path) in &layers {
        let layer_env = read_env_from_file(path);
        if layer_env.is_empty() {
            continue;
        }
        println!("  \x1b[2m[{label}]\x1b[0m  {}", path.display());
        for (key, raw) in &layer_env {
            let pad = " ".repeat(key_w.saturating_sub(key.len()));
            let override_tag = if printed.contains(key) {
                "  \x1b[33m(overridden by earlier layer)\x1b[0m"
            } else {
                ""
            };
            println!("    \x1b[32m●\x1b[0m  {key}{pad}  {raw}{override_tag}");
            printed.insert(key.clone());
        }
        println!();
    }

    Ok(())
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

fn read_env_from_file(path: &Path) -> Vec<(String, String)> {
    if !path.is_file() {
        return vec![];
    }
    let Ok(text) = fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };
    let Some(obj) = val.get("env").and_then(|v| v.as_object()) else {
        return vec![];
    };
    let mut out: Vec<(String, String)> = obj
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ── scope helpers ─────────────────────────────────────────────────────────────

fn detect_scope() -> Result<OrbitScope> {
    resolver::resolve_from_cwd().map_err(|_| {
        anyhow::anyhow!(
            "could not detect scope from current directory\n\
             cd into a workspace or use --scope to specify the target level"
        )
    })
}

fn detect_scope_for_list(scope_override: Option<ScopeLevel>) -> Result<OrbitScope> {
    let _ = scope_override; // for list we always load merged (all layers)
    detect_scope()
}

fn resolve_write_scope(
    scope_override: Option<ScopeLevel>,
) -> Result<(Option<OrbitScope>, ScopeLevel)> {
    let scope = resolver::resolve_from_cwd().map_err(|_| {
        anyhow::anyhow!(
            "could not detect scope from current directory\n\
             cd into a workspace or use --scope to specify the target level"
        )
    })?;
    let level = scope_override.unwrap_or_else(|| default_level(&scope));
    validate_level(&scope, level)?;
    Ok((Some(scope), level))
}

fn default_level(scope: &OrbitScope) -> ScopeLevel {
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

fn validate_level(scope: &OrbitScope, level: ScopeLevel) -> Result<()> {
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

fn orbit_json_path(scope: Option<&OrbitScope>, level: ScopeLevel) -> PathBuf {
    let scope = scope.unwrap();
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

/// All orbit.json paths in merge order (lowest to highest priority).
fn env_layers(scope: &OrbitScope) -> Vec<(String, PathBuf)> {
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
