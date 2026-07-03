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
    /// All other keys (model, agent, compaction, …) — last writer wins.
    pub extra: serde_json::Map<String, serde_json::Value>,
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
    pub mcp_servers: Vec<(String, Vec<String>)>,
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

    let global_opencode = dirs_global_config().join("opencode/opencode.jsonc");
    config_layers.push(LayerEntry {
        exists: global_opencode.is_file(),
        path: shorten_path(&home, &global_opencode),
        label: "opencode global".into(),
    });

    if !scope.global_mode {
        let global = scope.global_ai_root.as_path();
        let local = scope.ai_context_root.as_path();
        let same_root = local.canonicalize().ok() == global.canonicalize().ok();

        // helper: push one or two entries (global + optional local) for a dir pair
        let push_config_dual =
            |layers: &mut Vec<LayerEntry>, shared: &Path, loc: &Path, level: &str| {
                let find_config = |dir: &Path| -> (PathBuf, bool) {
                    let found = config_candidates(engine)
                        .iter()
                        .map(|c| dir.join(c))
                        .find(|p| p.is_file());
                    let path = found
                        .clone()
                        .unwrap_or_else(|| dir.join("opencode.json"));
                    (path, found.is_some())
                };

                let (sp, se) = find_config(shared);
                let same = loc.canonicalize().ok() == shared.canonicalize().ok();
                if same {
                    layers.push(LayerEntry {
                        exists: se,
                        path: shorten_path(&home, &sp),
                        label: level.to_string(),
                    });
                } else {
                    layers.push(LayerEntry {
                        exists: se,
                        path: shorten_path(&home, &sp),
                        label: format!("{level} (global)"),
                    });
                    let (lp, le) = find_config(loc);
                    layers.push(LayerEntry {
                        exists: le,
                        path: shorten_path(&home, &lp),
                        label: format!("{level} (workspace)"),
                    });
                }
            };

        push_config_dual(&mut config_layers, global, local, "workspace");

        let global_tenant = global.join("tenants").join(&scope.tenant);
        push_config_dual(
            &mut config_layers,
            &global_tenant,
            &scope.tenant_dir,
            "tenant",
        );

        if !scope.project.is_empty() {
            let global_project = global
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            let local_project = local
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            push_config_dual(&mut config_layers, &global_project, &local_project, "project");

            if !scope.repository.is_empty() {
                let global_repo = global_project.join("repositories").join(&scope.repository);
                let local_repo = local_project.join("repositories").join(&scope.repository);
                push_config_dual(&mut config_layers, &global_repo, &local_repo, "repo");
            }
        }

        // suppress unused warning when same_root (no dual entries emitted)
        let _ = same_root;
    }

    let global_cfg = scope.global_ai_root.join("opencode.json");
    config_layers.push(LayerEntry {
        exists: global_cfg.is_file(),
        path: shorten_path(&home, &global_cfg),
        label: "global root (always wins)".into(),
    });

    // ── MCP layers ────────────────────────────────────────────────────────────
    let mut mcp_layers: Vec<LayerEntry> = Vec::new();

    let catalog_mcp = dirs_global_config().join("orbit/mcps.json");
    mcp_layers.push(LayerEntry {
        exists: catalog_mcp.is_file(),
        path: shorten_path(&home, &catalog_mcp),
        label: "catalog".into(),
    });

    let push_dual = |layers: &mut Vec<LayerEntry>,
                     shared: &Path,
                     local: &Path,
                     rel: &str,
                     label: &str| {
        let sp = shared.join(rel);
        let lp = local.join(rel);
        layers.push(LayerEntry {
            exists: sp.is_file(),
            path: shorten_path(&home, &sp),
            label: format!("{label} (shared)"),
        });
        if lp.canonicalize().ok() != sp.canonicalize().ok() {
            layers.push(LayerEntry {
                exists: lp.is_file(),
                path: shorten_path(&home, &lp),
                label: format!("{label} (local)"),
            });
        }
    };

    let shared = scope.global_ai_root.as_path();
    let local = scope.ai_context_root.as_path();
    push_dual(&mut mcp_layers, shared, local, "mcp.json", "workspace");

    if !scope.global_mode {
        let tenant_rel = format!("tenants/{}/mcp.json", scope.tenant);
        push_dual(&mut mcp_layers, shared, local, &tenant_rel, "tenant");

        if !scope.project.is_empty() {
            let proj_rel = format!(
                "tenants/{}/projects/{}/mcp.json",
                scope.tenant, scope.project
            );
            push_dual(&mut mcp_layers, shared, local, &proj_rel, "project");

            if !scope.repository.is_empty() {
                let repo_rel = format!(
                    "tenants/{}/projects/{}/repositories/{}/mcp.json",
                    scope.tenant, scope.project, scope.repository
                );
                push_dual(&mut mcp_layers, shared, local, &repo_rel, "repo");
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
            .join("source-of-truth/opencode");
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
                .join("source-of-truth/opencode");
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
                    .join("source-of-truth/opencode");
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

    let mut mcp_servers: Vec<(String, Vec<String>)> = merged
        .mcp
        .iter()
        .map(|(name, srv)| (name.clone(), srv.command.clone()))
        .collect();
    mcp_servers.sort_by(|a, b| a.0.cmp(&b.0));

    ScopeReport {
        config_layers,
        mcp_layers,
        agent_overlay_dirs,
        instructions,
        mcp_servers,
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

/// Load and merge config from all scope layers for the given engine.
///
/// Loading order (each layer wins over the previous):
/// 1. `~/.config/opencode/opencode.jsonc`  (global opencode config)
/// 2. workspace config (`ai_context_root/opencode.json` or tenant-level)
/// 3. Dual-layer scope configs: for each scope level (workspace → tenant →
///    project → repo), `global_ai_root` is merged first, then
///    `ai_context_root` (so workspace-specific config wins over global).
///    When both roots point to the same directory only one pass runs.
/// 4. `global_ai_root/opencode.json`       (always last → always wins)
///
/// MCP servers are loaded from `mcp.json` files at each layer (same dual
/// pattern — mirrors this function exactly).
pub fn load(scope: &OrbitScope, engine: Engine) -> Result<MergedConfig> {
    let mut cfg = MergedConfig::default();

    // 1. Global opencode config (~/.config/opencode/opencode.jsonc)
    let global_opencode = dirs_global_config().join("opencode/opencode.jsonc");
    if global_opencode.is_file() {
        let val = jsonc::load_file(&global_opencode);
        merge_value_into(&mut cfg, val, &global_opencode, engine);
    }

    // 2. Workspace / tenant config
    let ws_config = if scope.global_mode {
        scope.ai_context_root.join("opencode.json")
    } else {
        scope.tenant_dir.join("opencode.json")
    };
    merge_file_into(&mut cfg, &ws_config, engine);

    // 3. Scope layers — dual pattern: global_ai_root first, then
    //    ai_context_root at every level so workspace-specific config wins.
    //    When both roots are the same directory, only one pass runs.
    if !scope.global_mode {
        let global = &scope.global_ai_root;
        let local = &scope.ai_context_root;

        // workspace root
        merge_layer_dual(&mut cfg, global, local, engine);

        // tenant
        let global_tenant = global.join("tenants").join(&scope.tenant);
        merge_layer_dual(&mut cfg, &global_tenant, &scope.tenant_dir, engine);

        if !scope.project.is_empty() {
            let global_project = global
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            let local_project = local
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project);
            merge_layer_dual(&mut cfg, &global_project, &local_project, engine);

            if !scope.repository.is_empty() {
                let global_repo = global_project.join("repositories").join(&scope.repository);
                let local_repo = local_project.join("repositories").join(&scope.repository);
                merge_layer_dual(&mut cfg, &global_repo, &local_repo, engine);
            }
        }
    }

    // 4. Global AI root config (always wins)
    merge_file_into(
        &mut cfg,
        &scope.global_ai_root.join("opencode.json"),
        engine,
    );

    // Load MCP from mcp.json files at each layer
    load_mcp_layers(scope, &mut cfg.mcp);

    Ok(cfg)
}

// ── layer helpers ─────────────────────────────────────────────────────────────

/// Try every candidate filename for this engine in `dir` and merge the first hit.
fn merge_layer(cfg: &mut MergedConfig, dir: &Path, engine: Engine) {
    for candidate in config_candidates(engine) {
        let path = dir.join(candidate);
        if path.is_file() {
            merge_file_into(cfg, &path, engine);
        }
    }
}

/// Merge `shared` (global governance) then `local` (workspace-specific).
/// When both paths resolve to the same directory only one pass runs.
fn merge_layer_dual(cfg: &mut MergedConfig, shared: &Path, local: &Path, engine: Engine) {
    merge_layer(cfg, shared, engine);
    if local.canonicalize().ok() != shared.canonicalize().ok() {
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
    engine: Engine,
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
            "mcp" => {
                if let Some(servers) = value.as_object() {
                    for (name, server) in servers {
                        if let Some(normalized) = mcp::normalize(base_dir, server) {
                            cfg.mcp.insert(name.clone(), normalized);
                        }
                    }
                }
            }
            // Gemini uses mcpServers instead of mcp
            "mcpServers" if engine == Engine::Gemini => {
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
fn load_mcp_layers(scope: &OrbitScope, target: &mut HashMap<String, McpServer>) {
    // Catalog MCPs configured via `orbit setup` or `orbit mcp enable` — lowest priority baseline.
    let catalog_mcp = dirs_global_config().join("orbit/mcps.json");
    mcp::merge_file(target, &catalog_mcp);

    // Plugin MCPs enabled via `orbit plugins enable` — override catalog MCPs.
    let plugins_mcp = dirs_global_config().join("orbit/plugins.mcp.json");
    mcp::merge_file(target, &plugins_mcp);

    let shared = &scope.global_ai_root;
    let local = &scope.ai_context_root;

    merge_dual_mcp(target, shared, local, "mcp.json");

    if !scope.global_mode {
        let tenant_rel = format!("tenants/{}/mcp.json", scope.tenant);
        merge_dual_mcp(target, shared, local, &tenant_rel);

        if !scope.project.is_empty() {
            let proj_rel = format!(
                "tenants/{}/projects/{}/mcp.json",
                scope.tenant, scope.project
            );
            merge_dual_mcp(target, shared, local, &proj_rel);

            if !scope.repository.is_empty() {
                let repo_rel = format!(
                    "tenants/{}/projects/{}/repositories/{}/mcp.json",
                    scope.tenant, scope.project, scope.repository
                );
                merge_dual_mcp(target, shared, local, &repo_rel);
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
    // avoid merging the same file twice when shared == local
    if local.canonicalize().ok() != shared.canonicalize().ok() {
        mcp::merge_file(target, &local);
    }
}

// ── misc helpers ──────────────────────────────────────────────────────────────

/// Config file candidates to probe per engine, in priority order.
fn config_candidates(engine: Engine) -> &'static [&'static str] {
    match engine {
        Engine::Opencode => &[
            "opencode.jsonc",
            "opencode.json",
            ".opencode/opencode.jsonc",
            ".opencode/opencode.json",
        ],
        Engine::Gemini => &[
            "opencode.jsonc",
            "opencode.json",
            "gemini.jsonc",
            "gemini.json",
            ".gemini/settings.json",
        ],
        Engine::Claude => &[
            "opencode.jsonc",
            "opencode.json",
            "claude.json",
            "claude.jsonc",
            ".claude/settings.json",
        ],
    }
}



fn dirs_global_config() -> PathBuf {
    // Respect XDG_CONFIG_HOME if set, otherwise ~/.config
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

}
