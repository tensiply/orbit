/// Background auto-update: governance git pull + binary install.
///
/// Fire-and-forget — spawned as a tokio task. All failures are silent;
/// errors are appended to `~/.local/share/orbit/orbit.log`.
use orbit_core::{user_config::UserConfig, workspace_config::WorkspaceConfig};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use crate::update_check;

const CACHE_TTL_SECS: u64 = 86_400; // 24 h

// ── public API ────────────────────────────────────────────────────────────────

/// Spawn background update tasks and return immediately.
/// `no_update` — suppressed via `--no-update` flag for this invocation.
pub fn spawn(ws_cfg: WorkspaceConfig, user_cfg: UserConfig, no_update: bool) {
    if no_update || std::env::var("ORBIT_NO_UPDATE_CHECK").is_ok() {
        return;
    }
    tokio::spawn(async move {
        tokio::join!(
            pull_governance_if_due(&ws_cfg, &user_cfg),
            update_binary_if_due(&ws_cfg, &user_cfg),
        );
    });
}

/// Print and clear any pending "orbit was auto-updated" notification.
pub fn print_pending_notification() {
    let path = pending_notification_path();
    if !path.is_file() {
        return;
    }
    if let Ok(version) = std::fs::read_to_string(&path) {
        let version = version.trim();
        if !version.is_empty() {
            eprintln!("  orbit auto-updated to {version}");
        }
    }
    let _ = std::fs::remove_file(&path);
}

// ── governance pull ───────────────────────────────────────────────────────────

async fn pull_governance_if_due(ws_cfg: &WorkspaceConfig, user_cfg: &UserConfig) {
    let _ = ws_cfg; // may carry future governance config
    if !user_cfg.update.auto_update_governance {
        return;
    }

    let cache = governance_cache_path();
    if !cache_expired(&cache) {
        return;
    }

    let ai_root = user_cfg.ai_root_expanded();
    if !ai_root.join(".git").is_dir() {
        return;
    }

    if repo_has_local_changes(&ai_root) || repo_in_progress(&ai_root) {
        return;
    }

    let result = std::process::Command::new("git")
        .args(["-C", &ai_root.to_string_lossy(), "pull", "--ff-only"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if result.map(|s| s.success()).unwrap_or(false) {
        write_cache(&cache);
    } else {
        log_error("governance git pull failed");
    }
}

fn repo_has_local_changes(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["-C", &root.to_string_lossy(), "status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

fn repo_in_progress(root: &Path) -> bool {
    root.join(".git/MERGE_HEAD").exists()
        || root.join(".git/rebase-merge").exists()
        || root.join(".git/rebase-apply").exists()
}

// ── binary auto-update ────────────────────────────────────────────────────────

async fn update_binary_if_due(ws_cfg: &WorkspaceConfig, user_cfg: &UserConfig) {
    if !user_cfg.update.auto_update_binary {
        return;
    }

    let mode = crate::commands::mode::current_mode();
    if mode == "dev" {
        return;
    }

    let cache = binary_cache_path();
    if !cache_expired(&cache) {
        return;
    }

    let lock = lock_path();
    if lock.exists() {
        return;
    }

    let client = match reqwest::Client::builder()
        .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let tag = match update_check::fetch_latest_tag(&client).await {
        Ok(t) => t,
        Err(_) => return,
    };
    let latest = tag.trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");

    if !update_check::is_newer(latest, current) {
        write_cache(&binary_cache_path());
        return;
    }

    // Acquire lock
    if write_lock(&lock).is_err() {
        return;
    }

    let result = download_and_install_silent(ws_cfg, &client, &tag, user_cfg).await;
    let _ = std::fs::remove_file(&lock);

    match result {
        Ok(()) => {
            write_cache(&binary_cache_path());
            write_pending_notification(&tag);
        }
        Err(e) => log_error(&format!("binary auto-update failed: {e}")),
    }
}

async fn download_and_install_silent(
    ws_cfg: &WorkspaceConfig,
    client: &reqwest::Client,
    version: &str,
    user_cfg: &UserConfig,
) -> anyhow::Result<()> {
    use crate::commands::update::{download_with_progress, parse_checksum, sha256_hex};

    let Some(binary_url) = ws_cfg.binary_url_for_platform() else {
        anyhow::bail!("no binary URL");
    };
    let artifact_name = binary_url.rsplit('/').next().unwrap_or("orbit").to_string();
    let checksums_url = binary_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{base}/checksums.txt"))
        .unwrap_or_else(|| {
            "https://github.com/befraeloircorona/orbit/releases/latest/download/checksums.txt"
                .to_string()
        });

    // Download without progress output (background)
    let dl_client = reqwest::Client::builder()
        .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let checksums_text = dl_client.get(&checksums_url).send().await?.text().await?;
    let expected = parse_checksum(&checksums_text, &artifact_name)
        .ok_or_else(|| anyhow::anyhow!("artifact not in checksums"))?;

    let resp = dl_client.get(&binary_url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let bytes = download_with_progress(resp).await?;

    if sha256_hex(&bytes) != expected {
        anyhow::bail!("checksum mismatch");
    }

    let install_path = user_cfg.install_dir_expanded().join("orbit");
    let tmp = install_path.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, &install_path)?;

    let _ = client; // suppress unused warning
    let _ = version;
    Ok(())
}

// ── paths ─────────────────────────────────────────────────────────────────────

fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("orbit")
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share/orbit"))
            .unwrap_or_else(|| PathBuf::from("/tmp/orbit"))
    }
}

fn governance_cache_path() -> PathBuf {
    data_dir().join("governance_sync")
}

fn binary_cache_path() -> PathBuf {
    data_dir().join("binary_auto_update")
}

fn lock_path() -> PathBuf {
    data_dir().join("update.lock")
}

fn pending_notification_path() -> PathBuf {
    data_dir().join("pending_update")
}

// ── cache helpers ─────────────────────────────────────────────────────────────

fn cache_expired(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    modified
        .elapsed()
        .map(|e| e.as_secs() >= CACHE_TTL_SECS)
        .unwrap_or(true)
}

fn write_cache(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, b"");
}

fn write_lock(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, b"")
}

fn write_pending_notification(version: &str) {
    let path = pending_notification_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, version.as_bytes());
}

fn log_error(msg: &str) {
    let log = data_dir().join("orbit.log");
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
    {
        let _ = writeln!(f, "[auto-update] {msg}");
    }
}
