use anyhow::{Context, Result, bail};
use clap::Args;
use orbit_core::{user_config::UserConfig, workspace_config::WorkspaceConfig};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, os::unix::fs::PermissionsExt, process::Command};

use crate::update_check;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Only update governance configs (skip binary update)
    #[arg(long)]
    pub governance_only: bool,

    /// Only update the binary (skip governance sync)
    #[arg(long)]
    pub binary_only: bool,

    /// Print what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Install even if already on the latest version
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: UpdateArgs) -> Result<()> {
    let user_cfg = UserConfig::load();
    let ai_root = user_cfg.ai_root_expanded();
    let ws_cfg = WorkspaceConfig::load(&ai_root);

    let do_governance = !args.binary_only;
    let do_binary = !args.governance_only;

    // ── governance sync ───────────────────────────────────────────────────────
    if do_governance {
        if !ai_root.is_dir() {
            bail!(
                "AI root not found: {}\nRun `orbit init <url>` first.",
                ai_root.display()
            );
        }

        if !ai_root.join(".git").is_dir() {
            println!("  AI root is not a git repo — skipping governance sync.");
            println!("  (Run `orbit init <url>` to set up a governance repo)");
        } else if args.dry_run {
            println!(
                "  [dry-run] would run: git -C {} pull --ff-only",
                ai_root.display()
            );
        } else {
            println!("  Syncing governance configs...");
            let status = Command::new("git")
                .args(["-C", &ai_root.to_string_lossy(), "pull", "--ff-only"])
                .status()?;
            if status.success() {
                println!("  Governance configs up to date.");
            } else {
                bail!("git pull failed — check your network/SSH access");
            }
        }
    }

    // ── binary update ─────────────────────────────────────────────────────────
    if do_binary {
        if std::env::var("ORBIT_NO_UPDATE_CHECK").is_ok() && !args.force {
            println!();
            println!(
                "  Binary update skipped (ORBIT_NO_UPDATE_CHECK is set). Use --force to override."
            );
            return Ok(());
        }

        let Some(binary_url) = ws_cfg.binary_url_for_platform() else {
            println!();
            println!("  No binary update URL configured.");
            println!("  (Admin: set `update.binary_url` in <ai_root>/orbit.toml)");
            return Ok(());
        };

        let client = reqwest::Client::builder()
            .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client")?;

        // Check latest version
        print!("  Checking latest version... ");
        let _ = std::io::stdout().flush();
        let latest_tag = match update_check::fetch_latest_tag(&client).await {
            Ok(tag) => tag,
            Err(e) => {
                println!("failed");
                bail!("Could not fetch latest release info: {e}");
            }
        };
        let latest = latest_tag.trim_start_matches('v');
        println!("{latest_tag}");

        if !args.force && !update_check::is_newer(latest, CURRENT_VERSION) {
            println!("  Already on the latest version (v{CURRENT_VERSION}).");
            println!();
            return Ok(());
        }

        // Derive checksums URL from the same release
        let artifact_name = binary_url.rsplit('/').next().unwrap_or("orbit").to_string();
        let checksums_url = binary_url
            .rsplit_once('/')
            .map(|(base, _)| format!("{base}/checksums.txt"))
            .unwrap_or_else(|| {
                "https://github.com/tensiply/orbit/releases/latest/download/checksums.txt"
                    .to_string()
            });

        if args.dry_run {
            println!();
            println!("  [dry-run] orbit v{CURRENT_VERSION} → {latest_tag}");
            println!("  [dry-run] binary:    {binary_url}");
            println!("  [dry-run] checksums: {checksums_url}");
            println!();
            return Ok(());
        }

        println!("  Updating orbit v{CURRENT_VERSION} → {latest_tag}");
        println!();

        // Download without the short timeout — use a no-timeout client for large files
        let dl_client = reqwest::Client::builder()
            .user_agent(concat!("orbit-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build download client")?;

        update_binary(
            &dl_client,
            &binary_url,
            &checksums_url,
            &artifact_name,
            &latest_tag,
        )
        .await?;
    }

    println!();
    Ok(())
}

// ── binary download & install ─────────────────────────────────────────────────

async fn update_binary(
    client: &reqwest::Client,
    binary_url: &str,
    checksums_url: &str,
    artifact_name: &str,
    version: &str,
) -> Result<()> {
    // Step 1: fetch checksums.txt
    print!("  Fetching checksums... ");
    let _ = std::io::stdout().flush();
    let checksums_text = {
        let resp = client
            .get(checksums_url)
            .send()
            .await
            .context("failed to fetch checksums.txt")?;
        if !resp.status().is_success() {
            bail!("checksums.txt: server returned {}", resp.status());
        }
        resp.text().await.context("failed to read checksums.txt")?
    };
    let expected_sha256 = parse_checksum(&checksums_text, artifact_name)
        .with_context(|| format!("'{artifact_name}' not found in checksums.txt"))?;
    println!("ok");

    // Step 2: download binary with progress
    let resp = client
        .get(binary_url)
        .send()
        .await
        .context("failed to start binary download")?;
    if !resp.status().is_success() {
        bail!("binary download: server returned {}", resp.status());
    }
    let bytes = download_with_progress(resp).await?;

    // Step 3: verify checksum
    print!("  Verifying checksum... ");
    let _ = std::io::stdout().flush();
    let actual = sha256_hex(&bytes);
    if actual != expected_sha256 {
        println!("FAILED");
        bail!(
            "checksum mismatch — download may be corrupt\n  expected: {expected_sha256}\n  got:      {actual}"
        );
    }
    println!("ok");

    // Step 4: atomic replace
    let current_exe = std::env::current_exe()?;
    let tmp_path = current_exe.with_extension("tmp");

    fs::write(&tmp_path, &bytes).with_context(|| {
        format!(
            "failed to write temp file — is {} writable? Try running with sudo.",
            current_exe.parent().unwrap_or(&current_exe).display()
        )
    })?;
    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))?;
    fs::rename(&tmp_path, &current_exe).with_context(|| {
        format!(
            "failed to replace binary at {} — try running with sudo",
            current_exe.display()
        )
    })?;

    println!("  orbit {} installed → {}", version, current_exe.display());
    Ok(())
}

fn parse_checksum(checksums: &str, artifact: &str) -> Option<String> {
    // sha256sum format: "<hash>  <filename>" (two spaces)
    checksums.lines().find_map(|line| {
        let (hash, name) = line.split_once("  ")?;
        if name.trim() == artifact {
            Some(hash.trim().to_string())
        } else {
            None
        }
    })
}

async fn download_with_progress(resp: reqwest::Response) -> Result<Vec<u8>> {
    use futures_util::StreamExt;

    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    let mut downloaded = 0u64;
    let mut last_pct = u64::MAX;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("error reading response body")?;
        buf.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        if let Some(total) = total {
            let pct = downloaded * 100 / total;
            if pct / 5 != last_pct / 5 {
                print!("\r  Downloading... {pct}%   ");
                let _ = std::io::stdout().flush();
                last_pct = pct;
            }
        }
    }

    if total.is_some() {
        println!("\r  Downloading... 100%  ");
    } else {
        println!("  Downloaded {} bytes", buf.len());
    }

    Ok(buf)
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_checksum_finds_artifact() {
        let txt = "abc123  orbit-linux-x86_64\ndef456  orbit-linux-aarch64\n";
        assert_eq!(
            parse_checksum(txt, "orbit-linux-x86_64"),
            Some("abc123".to_string())
        );
        assert_eq!(
            parse_checksum(txt, "orbit-linux-aarch64"),
            Some("def456".to_string())
        );
    }

    #[test]
    fn parse_checksum_missing_artifact() {
        let txt = "abc123  orbit-linux-x86_64\n";
        assert!(parse_checksum(txt, "orbit-linux-aarch64").is_none());
    }

    #[test]
    fn parse_checksum_trims_trailing_whitespace() {
        // sha256sum adds no leading spaces; trailing whitespace or \r\n should be ignored
        let txt = "abc123  orbit-linux-x86_64\r\n";
        assert_eq!(
            parse_checksum(txt, "orbit-linux-x86_64"),
            Some("abc123".to_string())
        );
    }
}
