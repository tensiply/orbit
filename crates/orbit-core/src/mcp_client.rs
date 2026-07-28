use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

// ── JSON-RPC 2.0 types ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Value,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    id: Option<Value>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// ── MCP server config (parsed from mcp.json entry) ───────────────────────────

#[derive(Deserialize, Clone, Debug)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

// ── Session (single subprocess, reused for init + call) ──────────────────────

struct McpSession {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpSession {
    fn start(cfg: &McpServerConfig) -> Result<Self> {
        let mut cmd = Command::new(&cfg.command);
        cmd.args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().with_context(|| {
            format!("failed to spawn MCP server '{}'", cfg.command)
        })?;

        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let reader = BufReader::new(stdout);

        Ok(McpSession { child, stdin, reader, next_id: 1 })
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let line = serde_json::to_string(&req)? + "\n";
        self.stdin.write_all(line.as_bytes()).context("write to MCP stdin")?;
        self.stdin.flush().context("flush MCP stdin")?;

        // Read lines until we get the response for our id
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = self.reader.read_line(&mut buf).context("read from MCP stdout")?;
            if n == 0 {
                bail!("MCP server closed stdout unexpectedly");
            }
            let trimmed = buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            let resp: JsonRpcResponse = serde_json::from_str(trimmed)
                .with_context(|| format!("invalid JSON-RPC response: {trimmed}"))?;

            if let Some(err) = resp.error {
                bail!("MCP error {}: {}", err.code, err.message);
            }
            return resp.result.context("MCP response has neither result nor error");
        }
    }

    fn initialize(&mut self) -> Result<()> {
        self.send_request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "clientInfo": { "name": "orbit", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {}
            }),
        )?;
        // Send initialized notification (no id, no response expected)
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let line = serde_json::to_string(&notif)? + "\n";
        self.stdin.write_all(line.as_bytes()).context("write initialized notification")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn call_tool(&mut self, tool: &str, params: Value) -> Result<Value> {
        let result = self.send_request(
            "tools/call",
            serde_json::json!({ "name": tool, "arguments": params }),
        )?;
        Ok(result)
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── MCP config loader ─────────────────────────────────────────────────────────

fn load_server_config(server_name: &str, mcp_config_path: &Path) -> Result<McpServerConfig> {
    let content = std::fs::read_to_string(mcp_config_path)
        .with_context(|| format!("reading MCP config at {}", mcp_config_path.display()))?;

    // Strip JSONC comments (single-line only)
    let stripped: String = content
        .lines()
        .map(|line| {
            if let Some(pos) = line.find("//") {
                // crude: skip if '//' appears outside a string value
                line[..pos].to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let root: Value = serde_json::from_str(&stripped)
        .with_context(|| format!("parsing MCP config at {}", mcp_config_path.display()))?;

    // Supports both flat { "servers": {...} } and Claude/OpenCode shape { "mcpServers": {...} }
    let servers = root
        .get("mcpServers")
        .or_else(|| root.get("servers"))
        .context("mcp config must have 'mcpServers' or 'servers' key")?;

    let entry = servers
        .get(server_name)
        .with_context(|| format!("server '{server_name}' not found in MCP config"))?;

    serde_json::from_value(entry.clone())
        .with_context(|| format!("parsing config for server '{server_name}'"))
}

// ── public API ────────────────────────────────────────────────────────────────

/// Call an MCP tool via JSON-RPC 2.0 over stdio.
///
/// Spawns the MCP server subprocess, sends `initialize`, then `tools/call`,
/// kills the subprocess, and returns the tool result. Timeout applies to the
/// entire call including subprocess startup. Default: 30 seconds.
pub fn call_mcp_tool(
    server_name: &str,
    tool: &str,
    params: Value,
    mcp_config_path: &Path,
    timeout: Option<Duration>,
) -> Result<Value> {
    let timeout = timeout.unwrap_or(Duration::from_secs(30));
    let cfg = load_server_config(server_name, mcp_config_path)?;

    // Run with timeout via a scoped thread
    let cfg_clone = cfg.clone();
    let tool_owned = tool.to_string();
    let params_clone = params.clone();

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let result = (|| -> Result<Value> {
            let mut session = McpSession::start(&cfg_clone)?;
            session.initialize()?;
            session.call_tool(&tool_owned, params_clone)
        })();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            bail!(
                "MCP call to '{server_name}/{tool}' timed out after {}s",
                timeout.as_secs()
            )
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            bail!("MCP call thread panicked for '{server_name}/{tool}'")
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_mcp_config(servers: &[(&str, &str, &[&str])]) -> NamedTempFile {
        let mut entries = serde_json::Map::new();
        for (name, cmd, args) in servers {
            entries.insert(
                name.to_string(),
                serde_json::json!({
                    "command": cmd,
                    "args": args
                }),
            );
        }
        let config = serde_json::json!({ "mcpServers": entries });
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", serde_json::to_string(&config).unwrap()).unwrap();
        f
    }

    #[test]
    fn load_server_config_found() {
        let f = write_mcp_config(&[("jira", "uvx", &["mcp-server-jira"])]);
        let cfg = load_server_config("jira", f.path()).unwrap();
        assert_eq!(cfg.command, "uvx");
        assert_eq!(cfg.args, vec!["mcp-server-jira"]);
    }

    #[test]
    fn load_server_config_missing_server() {
        let f = write_mcp_config(&[("jira", "uvx", &["mcp-server-jira"])]);
        let err = load_server_config("github", f.path()).unwrap_err();
        assert!(err.to_string().contains("github"), "error: {err}");
    }

    #[test]
    fn load_server_config_supports_servers_key() {
        let config = serde_json::json!({
            "servers": { "myserver": { "command": "echo", "args": [] } }
        });
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", serde_json::to_string(&config).unwrap()).unwrap();
        let cfg = load_server_config("myserver", f.path()).unwrap();
        assert_eq!(cfg.command, "echo");
    }
}
