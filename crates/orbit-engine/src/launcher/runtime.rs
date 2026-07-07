use anyhow::Result;
use orbit_core::{context::OrbitScope, engine::Engine};
use std::{fs, path::PathBuf};

/// All paths the launcher needs for a given engine invocation.
#[derive(Debug)]
pub struct RuntimePaths {
    /// Root of the isolated runtime directory (no cross-tenant state leaks here).
    pub runtime_dir: PathBuf,
    /// The config file the engine reads on startup.
    pub config_file: PathBuf,
    /// `XDG_CONFIG_HOME` override (keeps engine config isolated from ~/.config).
    pub xdg_config_home: PathBuf,
    /// `XDG_DATA_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME` overrides.
    pub xdg_data: PathBuf,
    pub xdg_cache: PathBuf,
    pub xdg_state: PathBuf,
}

/// Compute the engine config file path without creating any directories.
/// Used by dry-run to show what would be written.
pub fn config_file_path(scope: &OrbitScope, engine: Engine) -> PathBuf {
    let runtime_dir = runtime_root(scope, engine);
    match engine {
        Engine::Opencode => runtime_dir.join("config").join("opencode").join("opencode.jsonc"),
        Engine::Gemini => runtime_dir.join("config").join("gemini").join("settings.json"),
        Engine::Claude => runtime_dir.join("mcp-config.json"),
    }
}

/// Path where orbit writes the merged instruction content for a given engine.
/// - Claude: `context.md` (passed via `--append-system-prompt-file`)
/// - Gemini: `GEMINI.md` (picked up via `context.includeDirectories`)
/// - Opencode: `None` (instructions injected directly into config JSON)
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

    let xdg_config_home = runtime_dir.join("config");
    let xdg_data = runtime_dir.join("data");
    let xdg_cache = runtime_dir.join("cache");
    let xdg_state = runtime_dir.join("state");

    let engine_config_dir = xdg_config_home.join(engine.as_str());

    for dir in [&xdg_data, &engine_config_dir, &xdg_cache, &xdg_state] {
        fs::create_dir_all(dir)?;
    }

    let config_file = match engine {
        Engine::Opencode => engine_config_dir.join("opencode.jsonc"),
        Engine::Gemini => engine_config_dir.join("settings.json"),
        // Claude only needs MCPs in the config file; auth lives in ~/.claude
        Engine::Claude => runtime_dir.join("mcp-config.json"),
    };

    Ok(RuntimePaths {
        runtime_dir,
        config_file,
        xdg_config_home,
        xdg_data,
        xdg_cache,
        xdg_state,
    })
}

fn runtime_root(scope: &OrbitScope, engine: Engine) -> PathBuf {
    let suffix = format!(".{}-runtime", engine.as_str());
    if scope.global_mode {
        scope.ai_context_root.join(&suffix)
    } else {
        scope.tenant_dir.join(&suffix)
    }
}
