use crate::config::{McpServer, MergedConfig};
use orbit_core::engine::Engine;
use serde_json::{Map, Value, json};

/// Render `MergedConfig` into the JSON structure the target engine expects.
pub fn render(config: &MergedConfig, engine: Engine) -> Value {
    match engine {
        Engine::Opencode => render_opencode(config),
        Engine::Gemini => render_gemini(config),
        Engine::Claude => render_claude(config),
    }
}

// ── OpenCode ──────────────────────────────────────────────────────────────────
//
// Format:
//   {
//     "instructions": ["/abs/path/file.md", ...],
//     "mcp": { "name": { "type": "local", "command": [...], "environment": {...} } },
//     "model": "github-copilot/gpt-5.4-mini",
//     "agent": { "Planner": { "disable": true }, "Orchestrator": { "disable": true } },
//     ...extra keys
//   }

fn render_opencode(config: &MergedConfig) -> Value {
    let mut obj = config.extra.clone();

    // Instructions as absolute string paths
    let instructions: Vec<Value> = config
        .instructions
        .iter()
        .map(|p| Value::String(p.to_string_lossy().into_owned()))
        .collect();
    obj.insert("instructions".into(), Value::Array(instructions));

    // MCPs in opencode format: command as array, "environment" key
    let mcp_obj: Map<String, Value> = config
        .mcp
        .iter()
        .map(|(name, server)| (name.clone(), mcp_opencode(server)))
        .collect();
    obj.insert("mcp".into(), Value::Object(mcp_obj));

    // Force github-copilot model (mirrors bash launcher behaviour)
    let forced_model = "github-copilot/gpt-5.4-mini";
    obj.insert("model".into(), json!(forced_model));
    obj.insert("small_model".into(), json!(forced_model));
    obj.insert("enabled_providers".into(), json!(["github-copilot"]));
    obj.remove("disabled_providers");

    // Disable legacy orchestrator agents
    let agents = obj
        .entry("agent")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .unwrap();
    for legacy in ["Planner", "Orchestrator"] {
        agents
            .entry(legacy)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap()
            .insert("disable".into(), json!(true));
    }

    Value::Object(obj)
}

fn mcp_opencode(s: &McpServer) -> Value {
    if let Some(url) = &s.url {
        // OpenCode remote MCP format: { "type": "remote", "url": "...", "enabled": true }
        return json!({ "type": "remote", "url": url, "enabled": true });
    }
    let mut obj = json!({
        "type": s.server_type,
        "command": s.command,
    });
    if !s.environment.is_empty() {
        obj["environment"] = json!(s.environment);
    }
    if let Some(cwd) = &s.cwd {
        obj["cwd"] = json!(cwd.to_string_lossy().as_ref());
    }
    obj
}

// ── Gemini ────────────────────────────────────────────────────────────────────
//
// Format:
//   {
//     "mcpServers": { "name": { "command": "...", "args": [...], "env": {...} } },
//     "context": {
//       "includeDirectories": [...],
//       "loadFromIncludeDirectories": true,
//       "fileName": ["README.md", "GEMINI.md", "CONTEXT.md", "AGENTS.md"]
//     }
//   }

fn render_gemini(config: &MergedConfig) -> Value {
    let mut obj = config.extra.clone();
    obj.remove("instructions"); // Gemini uses includeDirectories instead
    obj.remove("mcp");

    // MCPs in Gemini/Claude format: command as string + args array
    let mcp_obj: Map<String, Value> = config
        .mcp
        .iter()
        .map(|(name, server)| (name.clone(), mcp_split(server)))
        .collect();
    obj.insert("mcpServers".into(), Value::Object(mcp_obj));

    // Build context.includeDirectories from instructions
    let include_dirs: Vec<Value> = config
        .instructions
        .iter()
        .filter_map(|p| p.parent())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|p| Value::String(p.to_string_lossy().into_owned()))
        .collect();

    let context = obj
        .entry("context")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();

    // Merge directories, keeping existing ones
    let existing: Vec<Value> = context
        .get("includeDirectories")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut all_dirs = existing;
    for d in include_dirs {
        if !all_dirs.contains(&d) {
            all_dirs.push(d);
        }
    }
    context.insert("includeDirectories".into(), Value::Array(all_dirs));
    context
        .entry("loadFromIncludeDirectories")
        .or_insert(json!(true));
    context.entry("fileName").or_insert(json!([
        "README.md",
        "GEMINI.md",
        "CONTEXT.md",
        "AGENTS.md"
    ]));

    Value::Object(obj)
}

// ── Claude ────────────────────────────────────────────────────────────────────
//
// mcp-config.json only holds MCPs — Claude reads auth from ~/.claude directly.
// Format: { "mcpServers": { "name": { "command": "...", "args": [...], "env": {...} } } }

fn render_claude(config: &MergedConfig) -> Value {
    let mcp_obj: Map<String, Value> = config
        .mcp
        .iter()
        .map(|(name, server)| (name.clone(), mcp_split(server)))
        .collect();
    json!({ "mcpServers": mcp_obj })
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Render a server in the split format used by Gemini and Claude:
/// `{ "command": "bin", "args": [...rest], "env": {...} }`
/// Remote servers render as `{ "type": "http", "url": "..." }`.
fn mcp_split(s: &McpServer) -> Value {
    if let Some(url) = &s.url {
        return json!({ "type": s.server_type, "url": url });
    }
    let (cmd, args) = s
        .command
        .split_first()
        .map_or(("", &[][..]), |(h, t)| (h.as_str(), t));
    let mut obj = json!({ "command": cmd });
    if !args.is_empty() {
        obj["args"] = json!(args);
    }
    if !s.environment.is_empty() {
        obj["env"] = json!(s.environment);
    }
    if let Some(cwd) = &s.cwd {
        obj["cwd"] = json!(cwd.to_string_lossy().as_ref());
    }
    obj
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MergedConfig;
    use std::{collections::HashMap, path::PathBuf};

    fn server(command: &[&str]) -> McpServer {
        McpServer {
            command: command.iter().map(|s| s.to_string()).collect(),
            environment: HashMap::new(),
            cwd: None,
            server_type: "local".into(),
            url: None,
        }
    }

    fn config_with_mcp(name: &str, cmd: &[&str]) -> MergedConfig {
        let mut cfg = MergedConfig::default();
        cfg.mcp.insert(name.to_string(), server(cmd));
        cfg
    }

    #[test]
    fn opencode_mcp_uses_array_command() {
        let cfg = config_with_mcp("srv", &["npx", "-y", "mcp-server"]);
        let val = render(&cfg, Engine::Opencode);
        let cmd = &val["mcp"]["srv"]["command"];
        assert!(cmd.is_array());
        assert_eq!(cmd[0], "npx");
    }

    #[test]
    fn opencode_disables_legacy_agents() {
        let cfg = MergedConfig::default();
        let val = render(&cfg, Engine::Opencode);
        assert_eq!(val["agent"]["Planner"]["disable"], true);
        assert_eq!(val["agent"]["Orchestrator"]["disable"], true);
    }

    #[test]
    fn opencode_forces_copilot_model() {
        let val = render(&MergedConfig::default(), Engine::Opencode);
        assert_eq!(val["model"], "github-copilot/gpt-5.4-mini");
    }

    #[test]
    fn gemini_splits_command_and_args() {
        let cfg = config_with_mcp("srv", &["npx", "-y", "mcp-server"]);
        let val = render(&cfg, Engine::Gemini);
        assert_eq!(val["mcpServers"]["srv"]["command"], "npx");
        assert_eq!(val["mcpServers"]["srv"]["args"][0], "-y");
    }

    #[test]
    fn claude_only_has_mcp_servers() {
        let mut cfg = config_with_mcp("srv", &["node", "index.js"]);
        cfg.extra.insert("model".into(), json!("some-model"));
        let val = render(&cfg, Engine::Claude);
        // Claude config must only contain mcpServers — no model leak
        assert!(val.get("model").is_none());
        assert!(val.get("mcpServers").is_some());
    }

    #[test]
    fn gemini_builds_include_dirs_from_instructions() {
        let mut cfg = MergedConfig::default();
        cfg.instructions
            .push(PathBuf::from("/home/user/AI/source-of-truth/README.md"));
        let val = render(&cfg, Engine::Gemini);
        let dirs = val["context"]["includeDirectories"].as_array().unwrap();
        assert!(
            dirs.iter()
                .any(|d| d.as_str().unwrap().contains("source-of-truth"))
        );
    }

    #[test]
    fn remote_mcp_renders_as_http_url_for_claude() {
        let mut cfg = MergedConfig::default();
        cfg.mcp.insert(
            "lucid".into(),
            McpServer {
                command: vec![],
                environment: HashMap::new(),
                cwd: None,
                server_type: "http".into(),
                url: Some("https://mcp.lucid.app/mcp".into()),
            },
        );
        let val = render(&cfg, Engine::Claude);
        assert_eq!(val["mcpServers"]["lucid"]["type"], "http");
        assert_eq!(
            val["mcpServers"]["lucid"]["url"],
            "https://mcp.lucid.app/mcp"
        );
        assert!(val["mcpServers"]["lucid"].get("command").is_none());
    }

    #[test]
    fn remote_mcp_renders_as_remote_type_for_opencode() {
        let mut cfg = MergedConfig::default();
        cfg.mcp.insert(
            "lucid".into(),
            McpServer {
                command: vec![],
                environment: HashMap::new(),
                cwd: None,
                server_type: "http".into(),
                url: Some("https://mcp.lucid.app/mcp".into()),
            },
        );
        let val = render(&cfg, Engine::Opencode);
        assert_eq!(val["mcp"]["lucid"]["type"], "remote");
        assert_eq!(val["mcp"]["lucid"]["url"], "https://mcp.lucid.app/mcp");
        assert_eq!(val["mcp"]["lucid"]["enabled"], true);
    }
}
