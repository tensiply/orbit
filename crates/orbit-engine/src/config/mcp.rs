use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// A normalized MCP server entry.
#[derive(Debug, Clone)]
pub struct McpServer {
    /// Full command: binary + args merged into one list.
    pub command: Vec<String>,
    pub environment: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    /// Server type for opencode format ("local" by default).
    pub server_type: String,
}

/// Normalize a raw MCP server JSON object into a `McpServer`.
///
/// Handles the two formats that appear in the wild:
/// - opencode: `{ command: "npx", args: [...], env: {...} }`
/// - normalized: `{ command: ["npx", ...], environment: {...} }`
///
/// Relative `./` paths in `command` and `cwd` are resolved against `base_dir`.
pub fn normalize(base_dir: &Path, raw: &serde_json::Value) -> Option<McpServer> {
    let obj = raw.as_object()?;

    let server_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("local")
        .to_string();

    // ── build command list ────────────────────────────────────────────────────
    let command: Vec<String> = match obj.get("command") {
        // Already a list: ["npx", "-y", "..."]
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| resolve_relative(s, base_dir))
            .collect(),
        // String + optional args array: "npx" + ["-y", "..."]
        Some(serde_json::Value::String(cmd)) => {
            let mut parts = vec![resolve_relative(cmd, base_dir)];
            if let Some(serde_json::Value::Array(args)) = obj.get("args") {
                parts.extend(
                    args.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| resolve_relative(s, base_dir)),
                );
            }
            parts
        }
        _ => return None,
    };

    // ── environment ───────────────────────────────────────────────────────────
    // Accept both "environment" (normalized) and "env" (shorthand)
    let env_val = obj.get("environment").or_else(|| obj.get("env"));
    let environment = env_val
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    // ── cwd ───────────────────────────────────────────────────────────────────
    let cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| {
        let p = Path::new(s);
        if s.starts_with("./") || s.starts_with("../") {
            base_dir.join(p)
        } else {
            p.to_path_buf()
        }
    });

    Some(McpServer {
        command,
        environment,
        cwd,
        server_type,
    })
}

/// Merge all servers from an `mcp.json` file into `target`.
/// Format: `{ "mcpServers": { "name": { ... } } }`
pub fn merge_file(target: &mut HashMap<String, McpServer>, path: &Path) {
    let val = super::jsonc::load_file(path);
    let Some(servers) = val.get("mcpServers").and_then(|v| v.as_object()) else {
        return;
    };
    for (name, server) in servers {
        if let Some(normalized) = normalize(path.parent().unwrap_or(Path::new(".")), server) {
            target.insert(name.clone(), normalized);
        }
    }
}

fn resolve_relative(s: &str, base: &Path) -> String {
    if s.starts_with("./") || s.starts_with("../") {
        normalize_path(&base.join(s)).to_string_lossy().into_owned()
    } else {
        s.to_string()
    }
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
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn normalizes_string_command_with_args() {
        let raw = json!({ "command": "npx", "args": ["-y", "some-mcp"], "env": { "FOO": "bar" } });
        let server = normalize(Path::new("/base"), &raw).unwrap();
        assert_eq!(server.command, vec!["npx", "-y", "some-mcp"]);
        assert_eq!(server.environment["FOO"], "bar");
    }

    #[test]
    fn normalizes_array_command() {
        let raw = json!({ "command": ["node", "./dist/index.js"], "environment": { "X": "1" } });
        let server = normalize(Path::new("/base"), &raw).unwrap();
        assert_eq!(server.command[0], "node");
        // relative path in command resolved against base
        assert_eq!(server.command[1], "/base/dist/index.js");
    }

    #[test]
    fn resolves_relative_cwd() {
        let raw = json!({ "command": "node", "cwd": "./server" });
        let server = normalize(Path::new("/base"), &raw).unwrap();
        assert_eq!(server.cwd.unwrap(), Path::new("/base/server"));
    }

    #[test]
    fn resolves_relative_args() {
        let raw = json!({
            "command": "npx",
            "args": ["-y", "some-mcp", "--config", "./config/settings.yaml"]
        });
        let server = normalize(Path::new("/base"), &raw).unwrap();
        assert_eq!(server.command[0], "npx");
        assert_eq!(server.command[1], "-y");
        assert_eq!(server.command[3], "--config");
        // relative arg must be resolved to absolute path
        assert_eq!(server.command[4], "/base/config/settings.yaml");
    }

    #[test]
    fn prefers_environment_over_env() {
        let raw = json!({
            "command": "x",
            "env": { "OLD": "1" },
            "environment": { "NEW": "2" }
        });
        let server = normalize(Path::new("/"), &raw).unwrap();
        assert!(server.environment.contains_key("NEW"));
        assert!(!server.environment.contains_key("OLD"));
    }
}
