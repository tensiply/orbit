//! Scope-aware plugin enablement status.
//!
//! Derives, for a given `OrbitScope`, whether each plugin is enabled — reading
//! the scope's `mcp.json` layers rather than a per-scope state file. The scoped
//! `mcp.json` set is the single source of truth (see ADR-014). The path walk is
//! shared with the config loader via [`crate::config::scoped_mcp_layers`].

use orbit_core::{
    context::{OrbitScope, ScopeLevel},
    data_paths,
    plugin::{self, PluginInstances, PluginState},
};
use serde::Serialize;
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, Serialize)]
pub struct PluginScopeStatus {
    pub name: String,
    /// Whether the plugin declares any MCP servers (static or instance).
    pub has_mcp: bool,
    /// The scope level whose own `mcp.json` holds this plugin's entries
    /// (deepest match), or `None` when the plugin is not enabled anywhere.
    pub enabled_here: Option<ScopeLevel>,
    /// Whether the plugin is effectively active in this scope — here or
    /// inherited from an ancestor level (or the global plugin file).
    pub enabled_effective: bool,
}

/// Status of every known plugin for `scope`.
pub fn plugin_status_for_scope(scope: &OrbitScope) -> Vec<PluginScopeStatus> {
    let name_level = mcp_name_levels(scope);
    let instances = PluginInstances::load();
    let state = PluginState::load();

    plugin::load_all()
        .into_iter()
        .map(|p| {
            if !p.has_mcp() {
                // No MCP entries: only the global on/off flag applies; the
                // scope layers carry no signal for this plugin.
                let on = state.is_enabled(&p.name);
                return PluginScopeStatus {
                    name: p.name,
                    has_mcp: false,
                    enabled_here: on.then_some(ScopeLevel::Global),
                    enabled_effective: on,
                };
            }

            // Canonical entry names (static + `{plugin}-{instance}`), reusing the
            // same generator the enable path writes with.
            let entry_names: Vec<String> = plugin::build_mcp_entries(&p, &instances)
                .unwrap_or_default()
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            let static_names: Vec<String> = p.mcp.iter().map(|m| m.name.clone()).collect();

            // Static entries gate effectiveness; instance entries are additive.
            let gate: &[String] = if static_names.is_empty() {
                &entry_names
            } else {
                &static_names
            };
            let enabled_effective =
                !gate.is_empty() && gate.iter().all(|n| name_level.contains_key(n));

            // Deepest level that declares any of this plugin's entries.
            let enabled_here = entry_names
                .iter()
                .filter_map(|n| name_level.get(n).copied())
                .max_by_key(|level| level.depth());

            PluginScopeStatus {
                name: p.name,
                has_mcp: true,
                enabled_here,
                enabled_effective,
            }
        })
        .collect()
}

/// Map each MCP server name to the deepest scope level that declares it.
fn mcp_name_levels(scope: &OrbitScope) -> HashMap<String, ScopeLevel> {
    let mut name_level: HashMap<String, ScopeLevel> = HashMap::new();

    // Global level: where scope-less `orbit plugins enable` / `orbit mcp enable`
    // write (under orbit_home). Lowest precedence — do not overwrite deeper hits.
    for name in mcp_names(&plugin::plugins_mcp_path()) {
        name_level.insert(name, ScopeLevel::Global);
    }
    for name in mcp_names(&data_paths::orbit_home().join("mcps.json")) {
        name_level.entry(name).or_insert(ScopeLevel::Global);
    }

    // Workspace → repository: deeper levels overwrite, so the stored value ends
    // up being the deepest level that declares each name.
    for (level, paths) in crate::config::scoped_mcp_layers(scope) {
        for path in &paths {
            for name in mcp_names(path) {
                name_level.insert(name, level);
            }
        }
    }

    name_level
}

fn mcp_names(path: &Path) -> Vec<String> {
    crate::config::mcp::mcp_names_in_file(path)
}
