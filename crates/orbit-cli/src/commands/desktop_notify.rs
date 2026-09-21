//! Best-effort bridge from file generation to a running Orbit Desktop instance.
//!
//! After `orbit document|image|svg create` writes a file, we ask the desktop's MCP server
//! to open it in a tab. The desktop debug/MCP server writes a discovery file into the
//! channel's data root (`orbit-desktop-debug.json`) with the port it bound; we read that,
//! POST a JSON-RPC `tools/call` for the `open_file` tool, and swallow every error. When the
//! desktop isn't running — or has no MCP server — this is simply a silent no-op.
//!
//! The discovery file is channel-scoped, and so is `orbit_data_root()`, so a canary/dev CLI
//! only ever talks to a canary/dev desktop. This relies on the session carrying the right
//! `ORBIT_CHANNEL` (see `orbit-engine`'s `channel_env`).

use serde_json::json;

/// File kinds the desktop can open in a tab. Values match the `kind` the desktop's
/// `open_file` MCP tool expects.
#[derive(Clone, Copy)]
pub enum FileKind {
    Doc,
    Image,
    Svg,
}

impl FileKind {
    fn as_str(self) -> &'static str {
        match self {
            FileKind::Doc => "doc",
            FileKind::Image => "image",
            FileKind::Svg => "svg",
        }
    }
}

/// Ask a running Orbit Desktop (same channel) to open `id` in a tab.
///
/// Returns `true` when the request reached the desktop MCP server, so callers can skip the
/// OS file-explorer fallback. Best-effort: returns `false` on any failure (no desktop, no
/// discovery file, connection refused, timeout).
pub fn open_in_desktop(kind: FileKind, id: &str) -> bool {
    let Some(port) = discover_desktop_port() else {
        return false;
    };
    // `run` handlers are synchronous but execute on the tokio runtime; block just this call.
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(post_open_file(port, kind, id))
    })
    .is_ok()
}

/// Port of this channel's desktop MCP server, read from its discovery file.
fn discover_desktop_port() -> Option<u16> {
    let path = orbit_core::data_paths::orbit_data_root().join("orbit-desktop-debug.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&raw).ok()?;
    doc.get("port")
        .and_then(|p| p.as_u64())
        .and_then(|p| u16::try_from(p).ok())
}

async fn post_open_file(port: u16, kind: FileKind, id: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(800))
        .build()?;
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "open_file",
            "arguments": { "kind": kind.as_str(), "id": id },
        },
    });
    client
        .post(format!("http://127.0.0.1:{port}/sse"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
