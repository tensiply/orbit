use anyhow::Result;
use orbit_core::{context::OrbitScope, engine::Engine};
use std::{fs, path::PathBuf};

/// All paths the launcher needs for a given engine invocation.
#[derive(Debug)]
pub struct RuntimePaths {
    /// Tenant-level runtime root — config, data, cache, state are isolated per tenant.
    pub runtime_dir: PathBuf,
    /// The config file the engine reads on startup.
    pub config_file: PathBuf,
    /// `XDG_CONFIG_HOME` — tenant-level (MCPs, engine config).
    pub xdg_config_home: PathBuf,
    /// `XDG_DATA_HOME` — tenant-level (opencode.db conversation history stays isolated).
    pub xdg_data: PathBuf,
    /// `XDG_CACHE_HOME` and `XDG_STATE_HOME` — tenant-level.
    pub xdg_cache: PathBuf,
    pub xdg_state: PathBuf,
    /// Workspace-level config dir — source of truth for gh auth (orbit auth copilot).
    pub workspace_config_dir: PathBuf,
    /// Workspace-level data dir — source of truth for opencode auth (orbit auth opencode).
    pub workspace_data_dir: PathBuf,
}

/// Compute the engine config file path without creating any directories.
/// Used by dry-run to show what would be written.
pub fn config_file_path(scope: &OrbitScope, engine: Engine) -> PathBuf {
    let runtime_dir = runtime_root(scope, engine);
    match engine {
        Engine::Opencode => runtime_dir
            .join("config")
            .join("opencode")
            .join("opencode.jsonc"),
        Engine::Gemini => runtime_dir
            .join("config")
            .join("gemini")
            .join("settings.json"),
        Engine::Claude => runtime_dir.join("mcp-config.json"),
    }
}

/// Path where orbit writes the merged instruction content for a given engine.
pub fn context_file_path(scope: &OrbitScope, engine: Engine) -> Option<PathBuf> {
    match engine {
        Engine::Claude => Some(runtime_root(scope, engine).join("context.md")),
        Engine::Gemini => Some(runtime_root(scope, engine).join("GEMINI.md")),
        Engine::Opencode => None,
    }
}

/// Create the runtime directory tree and return the resolved paths.
pub fn setup(scope: &OrbitScope, engine: Engine) -> Result<RuntimePaths> {
    let runtime_dir = runtime_root(scope, engine);
    let workspace_dir = workspace_runtime_root(scope, engine);

    let xdg_config_home = runtime_dir.join("config");
    let xdg_data = runtime_dir.join("data");
    let xdg_cache = runtime_dir.join("cache");
    let xdg_state = runtime_dir.join("state");
    let workspace_config_dir = workspace_dir.join("config");
    let workspace_data_dir = workspace_dir.join("data");

    let engine_config_dir = xdg_config_home.join(engine.as_str());

    for dir in [&xdg_data, &engine_config_dir, &xdg_cache, &xdg_state] {
        fs::create_dir_all(dir)?;
    }

    let config_file = match engine {
        Engine::Opencode => engine_config_dir.join("opencode.jsonc"),
        Engine::Gemini => engine_config_dir.join("settings.json"),
        Engine::Claude => runtime_dir.join("mcp-config.json"),
    };

    Ok(RuntimePaths {
        runtime_dir,
        config_file,
        xdg_config_home,
        xdg_data,
        xdg_cache,
        xdg_state,
        workspace_config_dir,
        workspace_data_dir,
    })
}

/// Copy workspace-level auth files into the tenant data dir right before launch.
///
/// opencode stores auth in `XDG_DATA_HOME/opencode/auth.json` but conversation
/// history in `XDG_DATA_HOME/opencode/opencode.db`. XDG_DATA_HOME stays
/// tenant-level so histories don't mix — auth is propagated here from the
/// workspace-level source set by `orbit auth`.
pub fn sync_workspace_auth(paths: &RuntimePaths) -> Result<()> {
    let ws_opencode = paths.workspace_data_dir.join("opencode");
    let tenant_opencode = paths.xdg_data.join("opencode");

    for file in ["auth.json", "mcp-auth.json"] {
        let src = ws_opencode.join(file);
        if src.exists() {
            fs::create_dir_all(&tenant_opencode)?;
            fs::copy(&src, tenant_opencode.join(file))?;
        }
    }
    Ok(())
}

/// Read the authenticated account from workspace-level auth for dry-run display.
pub fn workspace_auth_account(paths: &RuntimePaths) -> Option<String> {
    let auth_file = paths.workspace_data_dir.join("opencode").join("auth.json");
    let content = fs::read_to_string(&auth_file).ok()?;
    // auth.json is {"github-copilot": {"type":"oauth","access":"gho_...",...}}
    // Extract the provider key as the label.
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let provider = v.as_object()?.keys().next()?.clone();
    Some(provider)
}

/// Tenant-level runtime root.
fn runtime_root(scope: &OrbitScope, engine: Engine) -> PathBuf {
    runtime_dir_for_slug(scope, engine.as_str())
}

/// Workspace-level runtime root — always anchored at ai_context_root.
fn workspace_runtime_root(scope: &OrbitScope, engine: Engine) -> PathBuf {
    workspace_runtime_dir_for_slug(scope, engine.as_str())
}

/// Tenant-level runtime dir for a given engine slug.
pub fn runtime_dir_for_slug(scope: &OrbitScope, engine_slug: &str) -> PathBuf {
    let suffix = format!(".{engine_slug}-runtime");
    if scope.global_mode || scope.tenant.is_empty() {
        scope.ai_context_root.join(&suffix)
    } else {
        scope.tenant_dir.join(&suffix)
    }
}

/// Workspace-level runtime dir — always anchored at ai_context_root.
/// Used for auth so that all tenants in a workspace share the same account.
pub fn workspace_runtime_dir_for_slug(scope: &OrbitScope, engine_slug: &str) -> PathBuf {
    scope
        .ai_context_root
        .join(format!(".{engine_slug}-runtime"))
}
