use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use orbit_core::{plugin::OAuthSpec, secrets};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Run a full OAuth 2.1 PKCE + dynamic client registration flow.
/// Stores the resulting access token in the OS keychain under `spec.token_key`.
pub async fn run_oauth_flow(plugin_name: &str, spec: &OAuthSpec) -> Result<()> {
    let client = reqwest::Client::new();

    // ── 1. discover OAuth metadata ─────────────────────────────────────────────
    println!("  Discovering OAuth metadata…");
    let meta: serde_json::Value = client
        .get(&spec.discovery_url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to reach discovery URL: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("invalid discovery document: {e}"))?;

    let authorize_url = meta["authorization_endpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("discovery document missing 'authorization_endpoint'"))?;
    let token_url = meta["token_endpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("discovery document missing 'token_endpoint'"))?;
    let registration_url = meta["registration_endpoint"].as_str();

    // ── 2. bind local callback server ─────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // ── 3. dynamic client registration ────────────────────────────────────────
    let (client_id, client_secret) = if let Some(reg_url) = registration_url {
        println!("  Registering client dynamically…");
        let reg_resp: serde_json::Value = client
            .post(reg_url)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "client_name": format!("orbit / {plugin_name}"),
                "redirect_uris": [&redirect_uri],
                "grant_types": ["authorization_code"],
                "response_types": ["code"],
                "token_endpoint_auth_method": "none",
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("client registration failed: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("invalid registration response: {e}"))?;

        let id = reg_resp["client_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("registration response missing 'client_id'"))?
            .to_string();
        let secret = reg_resp["client_secret"]
            .as_str()
            .map(str::to_string);
        (id, secret)
    } else {
        // No dynamic registration — prompt for client_id
        println!("  No dynamic registration endpoint found.");
        println!("  Create an OAuth app at the service settings and provide the client credentials.");
        println!();
        let id = prompt_line("  Client ID")?;
        if id.is_empty() {
            bail!("client_id is required");
        }
        let secret = {
            let s = prompt_line("  Client Secret (leave blank if public client)")?;
            if s.is_empty() { None } else { Some(s) }
        };
        (id, secret)
    };

    // ── 4. PKCE ────────────────────────────────────────────────────────────────
    let code_verifier = generate_code_verifier();
    let code_challenge = pkce_challenge(&code_verifier);
    let state = generate_state();

    // ── 5. open browser ────────────────────────────────────────────────────────
    let auth_url = build_auth_url(
        authorize_url,
        &client_id,
        &redirect_uri,
        &spec.scope,
        &state,
        &code_challenge,
    );
    println!("  Opening browser for authorization…");
    println!("  \x1b[2mIf the browser does not open, visit:\x1b[0m");
    println!("  {auth_url}");
    println!();
    if let Err(e) = open::that(&auth_url) {
        eprintln!("  \x1b[33m!\x1b[0m  Could not open browser automatically: {e}");
    }

    // ── 6. wait for callback ───────────────────────────────────────────────────
    println!("  Waiting for authorization callback (2 min timeout)…");
    let code = wait_for_callback(listener, &state).await?;

    // ── 7. exchange code for token ────────────────────────────────────────────
    println!("  Exchanging authorization code for token…");
    let mut token_params = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", code_verifier),
    ];
    if let Some(secret) = client_secret {
        token_params.push(("client_secret", secret));
    }

    let token_resp = client
        .post(token_url)
        .form(&token_params)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("token request failed: {e}"))?;

    let status = token_resp.status();
    let body: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("invalid token response: {e}"))?;

    if !status.is_success() {
        let err = body["error"].as_str().unwrap_or("unknown");
        let desc = body["error_description"].as_str().unwrap_or("");
        bail!("token endpoint returned {status}: {err} — {desc}");
    }

    let access_token = body["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("token response missing 'access_token'"))?;

    // ── 8. store in keychain ──────────────────────────────────────────────────
    secrets::keychain_set(&spec.token_key, access_token)?;
    println!("  \x1b[32m✓\x1b[0m  Access token stored in keychain ({})", spec.token_key);

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn url_decode(s: &str) -> String {
    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                result.push(byte);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            result.push(b' ');
            i += 1;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

fn build_auth_url(
    base: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        base,
        url_encode(client_id),
        url_encode(redirect_uri),
        url_encode(scope),
        url_encode(state),
        url_encode(code_challenge),
    )
}

async fn wait_for_callback(
    listener: tokio::net::TcpListener,
    expected_state: &str,
) -> Result<String> {
    use tokio::time::{Duration, timeout};

    let (mut stream, _) = timeout(Duration::from_secs(120), listener.accept())
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for OAuth callback (2 min)"))?
        .map_err(|e| anyhow::anyhow!("callback server error: {e}"))?;

    // Read the HTTP request
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    let request = std::str::from_utf8(&buf[..n]).unwrap_or_default();

    // Parse path from first line: GET /callback?... HTTP/1.1
    let path = request
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");

    let query = path.splitn(2, '?').nth(1).unwrap_or("");
    // URL-decode values: authorization codes often contain percent-encoded chars.
    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), url_decode(v)))
        .collect();

    // Check for OAuth error from the provider
    if let Some(error) = params.get("error") {
        let desc = params.get("error_description").map(|s| s.as_str()).unwrap_or("");
        let html = format!(
            "<html><body><h1>Authorization failed</h1><p>{error}: {desc}</p></body></html>"
        );
        let _ = send_html_response(&mut stream, 400, &html).await;
        bail!("OAuth authorization failed: {error} — {desc}");
    }

    let state = params.get("state").map(|s| s.as_str()).unwrap_or("");
    if state != expected_state {
        let _ = send_html_response(&mut stream, 400, "<html><body><h1>State mismatch</h1></body></html>").await;
        bail!("state mismatch — possible CSRF attack");
    }

    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("callback missing 'code' parameter"))?;

    // Send success page
    let html = "<html><body><h1 style='font-family:sans-serif;color:#22c55e'>Authorization successful</h1><p style='font-family:sans-serif'>You can close this tab and return to the terminal.</p></body></html>";
    let _ = send_html_response(&mut stream, 200, html).await;

    Ok(code.to_string())
}

async fn send_html_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    html: &str,
) -> Result<()> {
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

fn prompt_line(prompt: &str) -> Result<String> {
    use std::io::{self, Write};
    print!("{prompt}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
