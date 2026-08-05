pub mod jsonc;
pub mod mcp;

use anyhow::Result;
use orbit_core::{context::OrbitScope, engine::Engine};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use mcp::McpServer;

// ── public types ──────────────────────────────────────────────────────────────

/// Merged config accumulated from all scope layers.
/// Engine-agnostic at this stage — rendering to opencode/gemini/claude format
/// happens in `orbit-engine::launcher`.
#[derive(Debug, Default, Clone)]
pub struct MergedConfig {
    /// Ordered instruction file paths (accumulated, no duplicates).
    pub instructions: Vec<PathBuf>,
    /// MCP servers keyed by name (last writer wins).
    pub mcp: HashMap<String, McpServer>,
    /// Extra environment variables injected into the engine process (last writer wins).
    pub env: HashMap<String, String>,
    /// Union of all `commands` arrays across scope layers.
    /// `None` = no scope layer declared a commands list → all commands materialised.
    /// `Some(set)` = only commands in this set are materialised.
    pub commands_filter: Option<std::collections::HashSet<String>>,
    /// All other keys (model, agent, compaction, …) — last writer wins.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl MergedConfig {
    /// Resolve secret prefixes (`secret://`, `keychain://`, `env://`, `file://`, `$VAR`)
    /// in every MCP server's `environment` and `headers` maps.
    /// Call this once, right before rendering to the engine config format.
    pub fn resolve_mcp_secrets(&mut self, workspace_slug: Option<&str>) {
        for server in self.mcp.values_mut() {
            for v in server.environment.values_mut() {
                *v = orbit_core::secrets::resolve_scoped(v, workspace_slug);
            }
            for v in server.headers.values_mut() {
                *v = orbit_core::secrets::resolve_scoped(v, workspace_slug);
            }
        }
    }
}

// ── scope inspection (dry-run) ────────────────────────────────────────────────

/// Status of a single config/MCP/overlay layer.
pub struct LayerEntry {
    pub path: PathBuf,
    pub exists: bool,
    pub label: String,
}

/// Full report of what each scope layer provides.
pub struct ScopeReport {
    pub config_layers: Vec<LayerEntry>,
    pub mcp_layers: Vec<LayerEntry>,
    pub agent_overlay_dirs: Vec<LayerEntry>,
    pub instructions: Vec<(PathBuf, bool)>,
    /// (name, command/url tokens, source layer label)
    pub mcp_servers: Vec<(String, Vec<String>, String)>,
    pub env_vars: Vec<(String, String)>,
    /// Commands that will be materialized: (name, source label).
    pub commands: Vec<(String, String)>,
    /// Enabled engine hooks (Claude only): (hook_name, events summary).
    pub engine_hooks: Vec<(String, String)>,
}

/// Load config AND build a layer-visibility report for dry-run output.
pub fn inspect(scope: &OrbitScope, engine: Engine) -> Result<(MergedConfig, ScopeReport)> {
    let merged = load(scope, engine)?;
    let report = build_scope_report(scope, engine, &merged);
    Ok((merged, report))
}

fn shorten_path(home: &Path, p: &Path) -> PathBuf {
    if let Ok(rel) = p.strip_prefix(home) {
        PathBuf::from("~").join(rel)
    } else {
        p.to_path_buf()
    }
}

fn build_scope_report(scope: &OrbitScope, engine: Engine, merged: &MergedConfig) -> ScopeReport {
    let home = dirs_home();

    // ── config layers (mirrors load() order) ─────────────────────────────────
    let mut config_layers: Vec<LayerEntry> = Vec::new();

    if engine == Engine::Opencode {
        let global_opencode = dirs_global_config().join("opencode/opencode.jsonc");
        config_layers.push(LayerEntry {
            exists: global_opencode.is_file(),
            path: shorten_path(&home, &global_opencode),
            label: "opencode global".into(),
        });
    }

    if !scope.global_mode {
        let global = scope.global_ai_root.as_path();
        let local = scope.ai_context_root.as_path();

        let find_config = |dir: &Path| -> (PathBuf, bool) {
            let found = config_candidates(engine)
                .iter()
                .map(|c| dir.join(c))
                .find(|p| p.is_file());
            let path = found.clone().unwrap_or_else(|| dir.join("orbit.json"));
            (path, found.is_some())
        };

        // workspace root — both global AI root and workspace AI root
        if local == global {
            let (p, e) = find_config(global);
            config_layers.push(LayerEntry {
                exists: e,
                path: shorten_path(&home, &p),
                label: "workspace".into(),
            });
        } else {
            let (gp, ge) = find_config(global);
            config_layers.push(LayerEntry {
                exists: ge,
                path: shorten_path(&home, &gp),
                label: "global".into(),
            });
            let (lp, le) = find_config(local);
            config_layers.push(LayerEntry {
                exists: le,
                path: shorten_path(&home, &lp),
                label: "workspace".into(),
            });
        }

        // tenant/project/repo — workspace AI root only (tenant config is workspace-scoped)
        {
            let (p, e) = find_config(&scope.tenant_dir);
            config_layers.push(LayerEntry {
                exists: e,
                path: shorten_path(&home, &p),
                label: "tenant".into(),
            });
        }

        if !scope.project.is_empty() {
            let proj_dir = local
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            let (p, e) = find_config(&proj_dir);
            config_layers.push(LayerEntry {
                exists: e,
                path: shorten_path(&home, &p),
                label: "project".into(),
            });

            if !scope.repository.is_empty() {
                let repo_dir = proj_dir.join("repositories").join(&scope.repository);
                let (p, e) = find_config(&repo_dir);
                config_layers.push(LayerEntry {
                    exists: e,
                    path: shorten_path(&home, &p),
                    label: "repo".into(),
                });
            }
        }
    }

    {
        let dir = scope.global_ai_root.as_path();
        let found = config_candidates(engine)
            .iter()
            .map(|c| dir.join(c))
            .find(|p| p.is_file());
        let path = found.clone().unwrap_or_else(|| dir.join("orbit.json"));
        config_layers.push(LayerEntry {
            exists: found.is_some(),
            path: shorten_path(&home, &path),
            label: "global root (always wins)".into(),
        });
    }

    // ── MCP layers ────────────────────────────────────────────────────────────
    let mut mcp_layers: Vec<LayerEntry> = Vec::new();

    let catalog_mcp = dirs_global_config().join("orbit/mcps.json");
    mcp_layers.push(LayerEntry {
        exists: catalog_mcp.is_file(),
        path: shorten_path(&home, &catalog_mcp),
        label: "catalog".into(),
    });

    let global_mcp = scope.global_ai_root.as_path();
    let ws_mcp = scope.ai_context_root.as_path();

    // workspace root — global AI root + workspace AI root
    if global_mcp == ws_mcp {
        mcp_layers.push(LayerEntry {
            exists: global_mcp.join("mcp.json").is_file(),
            path: shorten_path(&home, &global_mcp.join("mcp.json")),
            label: "workspace".into(),
        });
    } else {
        mcp_layers.push(LayerEntry {
            exists: global_mcp.join("mcp.json").is_file(),
            path: shorten_path(&home, &global_mcp.join("mcp.json")),
            label: "global".into(),
        });
        mcp_layers.push(LayerEntry {
            exists: ws_mcp.join("mcp.json").is_file(),
            path: shorten_path(&home, &ws_mcp.join("mcp.json")),
            label: "workspace".into(),
        });
    }

    if !scope.global_mode {
        // tenant/project/repo — workspace AI root only
        mcp_layers.push(LayerEntry {
            exists: ws_mcp
                .join("tenants")
                .join(&scope.tenant)
                .join("mcp.json")
                .is_file(),
            path: shorten_path(
                &home,
                &ws_mcp.join("tenants").join(&scope.tenant).join("mcp.json"),
            ),
            label: "tenant".into(),
        });

        if !scope.project.is_empty() {
            let proj_base = ws_mcp
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            mcp_layers.push(LayerEntry {
                exists: proj_base.join("mcp.json").is_file(),
                path: shorten_path(&home, &proj_base.join("mcp.json")),
                label: "project".into(),
            });

            if !scope.repository.is_empty() {
                let repo_base = proj_base.join("repositories").join(&scope.repository);
                mcp_layers.push(LayerEntry {
                    exists: repo_base.join("mcp.json").is_file(),
                    path: shorten_path(&home, &repo_base.join("mcp.json")),
                    label: "repo".into(),
                });
            }
        }
    }

    // ── agent overlay directories ─────────────────────────────────────────────
    let mut agent_overlay_dirs: Vec<LayerEntry> = Vec::new();

    if !scope.global_mode && !scope.tenant.is_empty() {
        let tenant_ov = scope
            .ai_context_root
            .join("tenants")
            .join(&scope.tenant)
            .join("source-of-truth/orbit");
        agent_overlay_dirs.push(LayerEntry {
            exists: tenant_ov.is_dir(),
            path: shorten_path(&home, &tenant_ov),
            label: "tenant".into(),
        });

        if !scope.project.is_empty() {
            let project_ov = scope
                .ai_context_root
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project)
                .join("source-of-truth/orbit");
            agent_overlay_dirs.push(LayerEntry {
                exists: project_ov.is_dir(),
                path: shorten_path(&home, &project_ov),
                label: "project".into(),
            });

            if !scope.repository.is_empty() {
                let repo_ov = scope
                    .ai_context_root
                    .join("tenants")
                    .join(&scope.tenant)
                    .join("projects")
                    .join(&scope.project)
                    .join("repositories")
                    .join(&scope.repository)
                    .join("source-of-truth/orbit");
                agent_overlay_dirs.push(LayerEntry {
                    exists: repo_ov.is_dir(),
                    path: shorten_path(&home, &repo_ov),
                    label: "repo".into(),
                });
            }
        }
    }

    // ── instructions + mcp from the already-merged config ────────────────────
    let instructions: Vec<(PathBuf, bool)> = merged
        .instructions
        .iter()
        .map(|p| (shorten_path(&home, p), p.is_file()))
        .collect();

    // Build source attribution: scan each mcp.json layer in priority order (last wins).
    // This mirrors the load_mcp_layers order so the label reflects where each server
    // was last defined — useful for dry-run debugging.
    let mcp_source = {
        let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        let scan = |path: &std::path::Path| -> Vec<String> {
            let Ok(text) = std::fs::read_to_string(path) else { return vec![] };
            let Ok(val) = jsonc::parse(&text) else { return vec![] };
            val.get("mcpServers")
                .and_then(|s| s.as_object())
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default()
        };

        // catalog (~/.orbit/mcps.json) and plugins (~/.orbit/plugins.mcp.json)
        for name in scan(&orbit_core::data_paths::orbit_home().join("mcps.json")) {
            map.insert(name, "catalog".into());
        }
        for name in scan(&orbit_core::data_paths::orbit_home().join("plugins.mcp.json")) {
            map.insert(name, "plugins".into());
        }
        // global AI root workspace mcp.json
        let global = scope.global_ai_root.as_path();
        let ws = scope.ai_context_root.as_path();
        for name in scan(&global.join("mcp.json")) {
            map.insert(name, "global".into());
        }
        if ws != global {
            for name in scan(&ws.join("mcp.json")) {
                map.insert(name, "workspace".into());
            }
        }
        if !scope.global_mode {
            // tenant
            let tp = ws.join("tenants").join(&scope.tenant).join("mcp.json");
            for name in scan(&tp) {
                map.insert(name, format!("tenant:{}", scope.tenant));
            }
            if !scope.project.is_empty() {
                let pp = ws
                    .join("tenants").join(&scope.tenant)
                    .join("projects").join(&scope.project)
                    .join("mcp.json");
                for name in scan(&pp) {
                    map.insert(name, format!("project:{}", scope.project));
                }
                if !scope.repository.is_empty() {
                    let rp = ws
                        .join("tenants").join(&scope.tenant)
                        .join("projects").join(&scope.project)
                        .join("repositories").join(&scope.repository)
                        .join("mcp.json");
                    for name in scan(&rp) {
                        map.insert(name, format!("repo:{}", scope.repository));
                    }
                }
            }
        }
        map
    };

    let mut mcp_servers: Vec<(String, Vec<String>, String)> = merged
        .mcp
        .iter()
        .map(|(name, srv)| {
            let display = if let Some(url) = &srv.url {
                vec![url.clone()]
            } else {
                srv.command.clone()
            };
            let source = mcp_source
                .get(name)
                .cloned()
                .unwrap_or_else(|| "inline".into());
            (name.clone(), display, source)
        })
        .collect();
    mcp_servers.sort_by(|a, b| a.0.cmp(&b.0));

    let mut env_vars: Vec<(String, String)> = merged
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    // ── commands ──────────────────────────────────────────────────────────────
    let filter = merged.commands_filter.as_ref();
    let mut commands: Vec<(String, String)> = Vec::new();

    for (name, _) in orbit_core::builtin_command::all() {
        if filter.is_none_or(|f| f.contains(*name)) {
            commands.push(((*name).to_string(), "built-in".to_string()));
        }
    }

    let user_cmds_dir = dirs_global_config().join("orbit/commands");
    if user_cmds_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&user_cmds_dir)
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path
                .file_stem()
                .and_then(|s| s.to_str().map(str::to_string))
            else {
                continue;
            };
            if orbit_core::builtin_command::find(&stem).is_some() {
                continue; // built-in already included
            }
            if filter.is_none_or(|f| f.contains(&stem)) {
                commands.push((stem, "user".to_string()));
            }
        }
    }
    commands.sort_by(|a, b| a.0.cmp(&b.0));

    // ── engine hooks (Claude only) ────────────────────────────────────────────
    let engine_hooks: Vec<(String, String)> = if engine == Engine::Claude {
        let state = orbit_core::engine_hook::EngineHookState::load();
        let catalog = orbit_core::engine_hook::load_all();
        let mut hooks: Vec<(String, String)> = catalog
            .iter()
            .filter(|e| state.is_enabled(&e.name))
            .map(|e| {
                let events = e
                    .events
                    .iter()
                    .map(|ev| ev.event.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                (e.name.clone(), events)
            })
            .collect();
        hooks.sort_by(|a, b| a.0.cmp(&b.0));
        hooks
    } else {
        vec![]
    };

    ScopeReport {
        config_layers,
        mcp_layers,
        agent_overlay_dirs,
        instructions,
        mcp_servers,
        env_vars,
        commands,
        engine_hooks,
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

/// Load and merge config from all scope layers for the given engine.
///
/// Loading order (each layer wins over the previous):
/// 1. `~/.config/opencode/opencode.jsonc`  (global opencode config, opencode engine only)
/// 2. Scope configs — global AI root then workspace AI root at each level:
///    - workspace root: dual-layer (global_ai_root then ai_context_root)
///    - tenant / project / repo: workspace AI root only (tenant config is
///      workspace-scoped — ~/AI does not hold workspace-specific tenants)
/// 3. `global_ai_root/orbit.json` (or opencode.json) — always last → always wins
///
/// `orbit.json` / `orbit.jsonc` take priority over legacy `opencode.json` names.
/// MCP servers are loaded from `mcp.json` files at each layer.
pub fn load(scope: &OrbitScope, engine: Engine) -> Result<MergedConfig> {
    let mut cfg = MergedConfig::default();

    // 1. Global opencode config — only for the opencode engine
    if engine == Engine::Opencode {
        let global_opencode = dirs_global_config().join("opencode/opencode.jsonc");
        if global_opencode.is_file() {
            let val = jsonc::load_file(&global_opencode);
            merge_value_into(&mut cfg, val, &global_opencode, engine);
        }
    }

    // 2. Scope configs
    if !scope.global_mode {
        let global = &scope.global_ai_root;
        let local = &scope.ai_context_root;

        // workspace root — both global and workspace AI root
        merge_layer_dual(&mut cfg, global, local, engine);

        // tenant and below — workspace AI root only
        merge_layer(&mut cfg, &scope.tenant_dir, engine);

        if !scope.project.is_empty() {
            let local_project = local
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            merge_layer(&mut cfg, &local_project, engine);

            if !scope.repository.is_empty() {
                let local_repo = local_project.join("repositories").join(&scope.repository);
                merge_layer(&mut cfg, &local_repo, engine);
            }
        }
    }

    // 3. Global AI root config (always wins — overrides tenant/project/repo)
    merge_layer(&mut cfg, &scope.global_ai_root, engine);

    // Load MCP from mcp.json files at each layer
    load_mcp_layers(scope, &mut cfg.mcp);

    // Re-apply inline MCPs from the global AI root so they win over everything,
    // including scope-level mcp.json files (which use "more-specific-wins" order).
    // load_mcp_layers runs after step 3, so without this step a repo mcp.json entry
    // would silently override a global orbit.json inline MCP.
    apply_global_root_inline_mcp(&mut cfg.mcp, &scope.global_ai_root, engine);

    Ok(cfg)
}

// ── layer helpers ─────────────────────────────────────────────────────────────

/// Load the highest-priority config file found in `dir` (first candidate that exists).
fn merge_layer(cfg: &mut MergedConfig, dir: &Path, engine: Engine) {
    for candidate in config_candidates(engine) {
        let path = dir.join(candidate);
        if path.is_file() {
            merge_file_into(cfg, &path, engine);
            return;
        }
    }
}

/// Merge `shared` (global governance) then `local` (workspace-specific).
/// When both paths resolve to the same directory only one pass runs.
fn merge_layer_dual(cfg: &mut MergedConfig, shared: &Path, local: &Path, engine: Engine) {
    merge_layer(cfg, shared, engine);
    if local != shared {
        merge_layer(cfg, local, engine);
    }
}

fn merge_file_into(cfg: &mut MergedConfig, path: &Path, engine: Engine) {
    if !path.is_file() {
        return;
    }
    let val = jsonc::load_file(path);
    merge_value_into(cfg, val, path, engine);
}

fn merge_value_into(
    cfg: &mut MergedConfig,
    val: serde_json::Value,
    source_path: &Path,
    _engine: Engine,
) {
    let Some(obj) = val.as_object() else { return };
    let base_dir = source_path.parent().unwrap_or(Path::new("."));

    for (key, value) in obj {
        match key.as_str() {
            "instructions" => {
                if let Some(arr) = value.as_array() {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            let p = if s.starts_with('/') {
                                PathBuf::from(s)
                            } else {
                                normalize_path(&base_dir.join(s))
                            };
                            if !cfg.instructions.contains(&p) {
                                cfg.instructions.push(p);
                            }
                        }
                    }
                }
            }
            "env" => {
                if let Some(obj) = value.as_object() {
                    for (k, v) in obj {
                        if let Some(s) = v.as_str() {
                            cfg.env.insert(k.clone(), s.to_string());
                        }
                    }
                }
            }
            "commands" => {
                if let Some(arr) = value.as_array() {
                    let names: std::collections::HashSet<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    match &mut cfg.commands_filter {
                        None => cfg.commands_filter = Some(names),
                        Some(set) => set.extend(names),
                    }
                }
            }
            // orbit/opencode format: `"mcp": { ... }`
            // All engines also recognise `"mcpServers"` (Gemini/Claude/native format)
            "mcp" | "mcpServers" => {
                if let Some(servers) = value.as_object() {
                    for (name, server) in servers {
                        if let Some(normalized) = mcp::normalize(base_dir, server) {
                            cfg.mcp.insert(name.clone(), normalized);
                        }
                    }
                }
            }
            _ => {
                cfg.extra.insert(key.clone(), value.clone());
            }
        }
    }
}

/// Load MCP from `mcp.json` files at every scope layer (shared + local pattern).
/// Re-apply inline `mcp`/`mcpServers` entries from the global AI root's config file,
/// overwriting any same-named entries that `load_mcp_layers` may have introduced.
///
/// `load_mcp_layers` follows "more-specific-wins" order (repo beats global), which
/// would otherwise silently override MCPs defined inline in the global orbit.json.
/// This call restores the "global always wins" invariant for inline MCPs.
fn apply_global_root_inline_mcp(
    target: &mut HashMap<String, McpServer>,
    global_root: &Path,
    engine: Engine,
) {
    for candidate in config_candidates(engine) {
        let path = global_root.join(candidate);
        if !path.is_file() {
            continue;
        }
        let val = jsonc::load_file(&path);
        let Some(obj) = val.as_object() else { return };
        for key in ["mcp", "mcpServers"] {
            if let Some(servers) = obj.get(key).and_then(|v| v.as_object()) {
                for (name, server) in servers {
                    if let Some(normalized) = mcp::normalize(global_root, server) {
                        target.insert(name.clone(), normalized);
                    }
                }
            }
        }
        return; // only the highest-priority config file in global_root
    }
}

fn load_mcp_layers(scope: &OrbitScope, target: &mut HashMap<String, McpServer>) {
    // Catalog MCPs configured via `orbit setup` or `orbit mcp enable` — lowest priority baseline.
    let catalog_mcp = dirs_global_config().join("orbit/mcps.json");
    mcp::merge_file(target, &catalog_mcp);

    // Plugin MCPs enabled via `orbit plugins enable` — override catalog MCPs.
    let plugins_mcp = dirs_global_config().join("orbit/plugins.mcp.json");
    mcp::merge_file(target, &plugins_mcp);

    let global = &scope.global_ai_root;
    let local = &scope.ai_context_root;

    // workspace root — both global and workspace AI root
    merge_dual_mcp(target, global, local, "mcp.json");

    if !scope.global_mode {
        // tenant and below — workspace AI root only
        mcp::merge_file(
            target,
            &local.join("tenants").join(&scope.tenant).join("mcp.json"),
        );

        if !scope.project.is_empty() {
            let proj_base = local
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            mcp::merge_file(target, &proj_base.join("mcp.json"));

            if !scope.repository.is_empty() {
                mcp::merge_file(
                    target,
                    &proj_base
                        .join("repositories")
                        .join(&scope.repository)
                        .join("mcp.json"),
                );
            }
        }
    }
}

fn merge_dual_mcp(
    target: &mut HashMap<String, McpServer>,
    shared_root: &Path,
    local_root: &Path,
    relative: &str,
) {
    let shared = shared_root.join(relative);
    let local = local_root.join(relative);
    mcp::merge_file(target, &shared);
    // avoid merging the same file twice when shared_root == local_root
    if local_root != shared_root {
        mcp::merge_file(target, &local);
    }
}

// ── misc helpers ──────────────────────────────────────────────────────────────

/// Config file candidates to probe per engine, in priority order.
/// `orbit.json` / `orbit.jsonc` take precedence over legacy `opencode.json` names.
fn config_candidates(engine: Engine) -> &'static [&'static str] {
    match engine {
        Engine::Opencode => &[
            "orbit.jsonc",
            "orbit.json",
            "opencode.jsonc",
            "opencode.json",
            ".opencode/opencode.jsonc",
            ".opencode/opencode.json",
        ],
        Engine::Gemini => &[
            "orbit.jsonc",
            "orbit.json",
            "opencode.jsonc",
            "opencode.json",
            "gemini.jsonc",
            "gemini.json",
            ".gemini/settings.json",
        ],
        Engine::Claude => &[
            "orbit.jsonc",
            "orbit.json",
            "opencode.jsonc",
            "opencode.json",
            "claude.json",
            "claude.jsonc",
            ".claude/settings.json",
        ],
    }
}

fn dirs_global_config() -> PathBuf {
    // ORBIT_CONFIG_HOME is set by the launcher before it overrides XDG_CONFIG_HOME
    // for session isolation. Use it to find the real user config dir.
    if let Ok(orbit_home) = std::env::var("ORBIT_CONFIG_HOME") {
        return PathBuf::from(orbit_home);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        dirs_home().join(".config")
    }
}

fn dirs_home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Resolve `.` and `..` components without hitting the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut parts: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn accumulates_instructions_no_duplicates() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "a.json",
            r#"{ "instructions": ["README.md", "docs.md"] }"#,
        );
        write(
            tmp.path(),
            "b.json",
            r#"{ "instructions": ["docs.md", "extra.md"] }"#, // "docs.md" is a dup
        );
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("a.json"), Engine::Opencode);
        merge_file_into(&mut cfg, &tmp.path().join("b.json"), Engine::Opencode);

        // docs.md should appear only once, extra.md added
        let names: Vec<_> = cfg
            .instructions
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.iter().filter(|n| *n == "docs.md").count(), 1);
        assert!(names.contains(&"extra.md".to_string()));
    }

    #[test]
    fn later_mcp_wins() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "a.json",
            r#"{ "mcp": { "server1": { "command": "old" } } }"#,
        );
        write(
            tmp.path(),
            "b.json",
            r#"{ "mcp": { "server1": { "command": "new" } } }"#,
        );
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("a.json"), Engine::Opencode);
        merge_file_into(&mut cfg, &tmp.path().join("b.json"), Engine::Opencode);

        assert_eq!(cfg.mcp["server1"].command[0], "new");
    }

    #[test]
    fn extra_keys_last_writer_wins() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "a.json", r#"{ "model": "fast" }"#);
        write(tmp.path(), "b.json", r#"{ "model": "smart" }"#);
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("a.json"), Engine::Opencode);
        merge_file_into(&mut cfg, &tmp.path().join("b.json"), Engine::Opencode);

        assert_eq!(cfg.extra["model"], "smart");
    }

    #[test]
    fn env_merges_last_writer_wins() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "a.json",
            r#"{ "env": { "FOO": "old", "BAR": "keep" } }"#,
        );
        write(tmp.path(), "b.json", r#"{ "env": { "FOO": "new" } }"#);
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("a.json"), Engine::Opencode);
        merge_file_into(&mut cfg, &tmp.path().join("b.json"), Engine::Opencode);

        assert_eq!(cfg.env["FOO"], "new");
        assert_eq!(cfg.env["BAR"], "keep");
    }

    #[test]
    fn resolves_relative_instructions_to_absolute() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "cfg.json",
            r#"{ "instructions": ["README.md"] }"#,
        );
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("cfg.json"), Engine::Opencode);

        // The instruction path should be absolute (resolved from cfg.json's dir)
        assert!(cfg.instructions[0].is_absolute());
        assert_eq!(cfg.instructions[0], tmp.path().join("README.md"));
    }

    // ── BUG 1 regression: mcpServers recognized for all engines ─────────────────

    #[test]
    fn mcp_servers_key_recognized_for_claude() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "cfg.json",
            r#"{ "mcpServers": { "gh": { "command": "gh", "args": ["mcp", "serve"] } } }"#,
        );
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("cfg.json"), Engine::Claude);
        assert!(cfg.mcp.contains_key("gh"), "mcpServers must be parsed for Claude");
    }

    #[test]
    fn mcp_servers_key_recognized_for_opencode() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "cfg.json",
            r#"{ "mcpServers": { "srv": { "command": "npx", "args": ["-y", "mcp-server"] } } }"#,
        );
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("cfg.json"), Engine::Opencode);
        assert!(cfg.mcp.contains_key("srv"), "mcpServers must be parsed for OpenCode");
    }

    #[test]
    fn mcp_and_mcp_servers_both_loaded_last_wins() {
        // A config with both keys — `mcpServers` is processed after `mcp` (map iteration
        // order), but both must contribute non-colliding entries.
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "cfg.json",
            r#"{
                "mcp": { "a": { "command": "bin-a" } },
                "mcpServers": { "b": { "command": "bin-b" } }
            }"#,
        );
        let mut cfg = MergedConfig::default();
        merge_file_into(&mut cfg, &tmp.path().join("cfg.json"), Engine::Claude);
        assert!(cfg.mcp.contains_key("a"));
        assert!(cfg.mcp.contains_key("b"));
    }

    // ── BUG 4 regression: global root inline MCPs win over scope mcp.json files ─

    #[test]
    fn global_inline_mcp_wins_over_repo_mcp_json() {
        use orbit_core::context::OrbitScope;

        let tmp = TempDir::new().unwrap();
        let global_root = tmp.path().join("global");
        let ws_root = tmp.path().join("ws");
        let repo_dir = ws_root
            .join("tenants").join("T")
            .join("projects").join("P")
            .join("repositories").join("R");
        std::fs::create_dir_all(&global_root).unwrap();
        std::fs::create_dir_all(&repo_dir).unwrap();

        // Global orbit.json defines "shared-server" with command "global-cmd"
        write(
            &global_root,
            "orbit.json",
            r#"{ "mcp": { "shared-server": { "command": "global-cmd" } } }"#,
        );
        // Repo mcp.json redefines the same server with a different command
        std::fs::write(
            repo_dir.join("mcp.json"),
            r#"{ "mcpServers": { "shared-server": { "command": "repo-cmd" } } }"#,
        ).unwrap();

        let scope = OrbitScope {
            global_ai_root: global_root.clone(),
            ai_context_root: ws_root.clone(),
            workspace_root: ws_root.clone(),
            tenant_dir: ws_root.join("tenants").join("T"),
            code_root: ws_root.clone(),
            work_dir: ws_root.clone(),
            tenant: "T".into(),
            project: "P".into(),
            repository: "R".into(),
            global_mode: false,
        };

        let cfg = load(&scope, Engine::Claude).unwrap();
        assert_eq!(
            cfg.mcp["shared-server"].command[0], "global-cmd",
            "global orbit.json inline MCP must win over repo mcp.json"
        );
    }

    #[test]
    fn apply_global_root_inline_mcp_is_noop_when_no_overlap() {
        use orbit_core::context::OrbitScope;

        let tmp = TempDir::new().unwrap();
        let global_root = tmp.path().join("global");
        let ws_root = tmp.path().join("ws");
        let repo_dir = ws_root
            .join("tenants").join("T")
            .join("projects").join("P")
            .join("repositories").join("R");
        std::fs::create_dir_all(&global_root).unwrap();
        std::fs::create_dir_all(&repo_dir).unwrap();

        // Different server names — no overlap
        write(
            &global_root,
            "orbit.json",
            r#"{ "mcp": { "global-only": { "command": "global-cmd" } } }"#,
        );
        std::fs::write(
            repo_dir.join("mcp.json"),
            r#"{ "mcpServers": { "repo-only": { "command": "repo-cmd" } } }"#,
        ).unwrap();

        let scope = OrbitScope {
            global_ai_root: global_root.clone(),
            ai_context_root: ws_root.clone(),
            workspace_root: ws_root.clone(),
            tenant_dir: ws_root.join("tenants").join("T"),
            code_root: ws_root.clone(),
            work_dir: ws_root.clone(),
            tenant: "T".into(),
            project: "P".into(),
            repository: "R".into(),
            global_mode: false,
        };

        let cfg = load(&scope, Engine::Claude).unwrap();
        // Both servers are present (no clobbering when names don't overlap)
        assert!(cfg.mcp.contains_key("global-only"));
        assert!(cfg.mcp.contains_key("repo-only"));
    }
}
