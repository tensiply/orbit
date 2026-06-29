use orbit_core::workspace_config::WorkspaceConfig;
use std::path::{Path, PathBuf};
use std::time::Duration;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CACHE_TTL_SECS: u64 = 86_400; // 24 hours
const TIMEOUT: Duration = Duration::from_secs(1);
pub const API_URL: &str = "https://api.github.com/repos/tensiply/orbit/releases/latest";
pub const API_RELEASES_URL: &str = "https://api.github.com/repos/tensiply/orbit/releases";

/// Checks GitHub for a newer release and prints a one-line notice if one exists.
/// All failures (network, timeout, parse) are silent — startup is never blocked.
pub async fn check_and_print(ws_cfg: &WorkspaceConfig) {
    if !ws_cfg.update.check_on_startup {
        return;
    }
    if std::env::var("ORBIT_NO_UPDATE_CHECK").is_ok() {
        return;
    }

    let cache = cache_path();
    if !cache_expired(&cache) {
        return;
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(TIMEOUT)
        .build()
        .unwrap_or_default();

    let result = tokio::time::timeout(TIMEOUT, fetch_latest_tag(&client)).await;

    if let Ok(Ok(tag)) = result {
        write_cache(&cache);
        let latest = tag.trim_start_matches('v');
        if is_newer(latest, CURRENT_VERSION) {
            eprintln!(
                "  orbit {} is available (you have {}). Run `orbit update` to upgrade.",
                latest, CURRENT_VERSION
            );
        }
    }
    // timeout or network error: no cache update, will retry next invocation
}

// ── version comparison ────────────────────────────────────────────────────────

pub fn is_newer(latest: &str, current: &str) -> bool {
    parse_semver(latest) > parse_semver(current)
}

fn parse_semver(v: &str) -> (u64, u64, u64) {
    let v = v.trim_start_matches('v');
    let mut parts = v.split('.').filter_map(|p| p.parse::<u64>().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

// ── GitHub API ────────────────────────────────────────────────────────────────

pub async fn fetch_latest_prerelease_tag(client: &reqwest::Client) -> anyhow::Result<String> {
    let resp = client.get(API_RELEASES_URL).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let body = resp.text().await?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    json.as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|r| r["prerelease"].as_bool().unwrap_or(false))
        })
        .and_then(|r| r["tag_name"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No pre-release found on GitHub"))
}

pub async fn fetch_latest_tag(client: &reqwest::Client) -> anyhow::Result<String> {
    let resp = client.get(API_URL).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let body = resp.text().await?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    json["tag_name"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("missing tag_name in response"))
}

// ── cache ─────────────────────────────────────────────────────────────────────

fn cache_path() -> PathBuf {
    let data_dir = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    };
    data_dir.join("orbit").join("update_check")
}

fn cache_expired(cache: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(cache) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    let Ok(elapsed) = modified.elapsed() else {
        return true;
    };
    elapsed.as_secs() >= CACHE_TTL_SECS
}

fn write_cache(cache: &Path) {
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(cache, b"");
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_detected() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.1.1", "0.1.0"));
    }

    #[test]
    fn same_version_not_newer() {
        assert!(!is_newer("0.1.0", "0.1.0"));
    }

    #[test]
    fn older_version_not_newer() {
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn v_prefix_stripped() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.2.0", "v0.1.0"));
    }
}
