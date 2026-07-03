use orbit_core::plugin::{self, Plugin, PluginState};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::config::MergedConfig;

/// Inject static context from enabled plugins into `config.instructions`.
/// Each plugin with `[context]` contributes:
///   - An inline `prompt` written to a temp file in `runtime_dir`
///   - Any explicit `instructions` file paths (~ expanded)
pub fn inject_context(
    state: &PluginState,
    plugins: &[Plugin],
    config: &mut MergedConfig,
    runtime_dir: &Path,
) -> anyhow::Result<()> {
    for p in plugins {
        if !state.is_enabled(&p.name) {
            continue;
        }
        let Some(ctx) = &p.context else { continue };

        if let Some(prompt) = &ctx.prompt {
            let path = runtime_dir.join(format!("plugin-context-{}.md", p.name));
            fs::write(&path, prompt)?;
            if !config.instructions.contains(&path) {
                config.instructions.push(path);
            }
        }

        for raw in &ctx.instructions {
            let path = expand_tilde(raw);
            if !config.instructions.contains(&path) {
                config.instructions.push(path);
            }
        }
    }
    Ok(())
}

/// Run pre-launch commands for all enabled plugins that declare `[pre_launch]`.
/// Returns extra instruction file paths generated from "context" output mode.
/// Each command is soft-fail: timeout or non-zero exit is warned and skipped.
pub fn run_pre_launch(
    state: &PluginState,
    plugins: &[Plugin],
    runtime_dir: &Path,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for p in plugins {
        if !state.is_enabled(&p.name) {
            continue;
        }
        let Some(spec) = &p.pre_launch else { continue };

        // Cache hit?
        if let Some(ttl) = spec.cache_ttl_secs {
            let cache = cache_path(&p.name);
            if let Some(cached) = read_if_fresh(&cache, ttl) {
                if spec.output == "context" {
                    if let Some(out) = write_context_file(&cached, &p.name, runtime_dir) {
                        paths.push(out);
                    }
                }
                continue;
            }
        }

        let timeout = Duration::from_secs(spec.timeout_secs.unwrap_or(5));
        match run_with_timeout(&spec.cmd, timeout) {
            None => tracing::warn!(
                "pre_launch '{}': timed out or failed — skipping",
                p.name
            ),
            Some(stdout) => {
                if spec.cache_ttl_secs.is_some() {
                    persist_cache(&p.name, &stdout);
                }
                match spec.output.as_str() {
                    "context" => {
                        if let Some(out) = write_context_file(&stdout, &p.name, runtime_dir) {
                            paths.push(out);
                        }
                    }
                    "env" => apply_env_lines(&stdout, &p.name),
                    _ => {} // "none" — side effects only
                }
            }
        }
    }

    paths
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn run_with_timeout(cmd_str: &str, timeout: Duration) -> Option<Vec<u8>> {
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let child = Command::new(parts[0])
        .args(&parts[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(out)) if out.status.success() => Some(out.stdout),
        _ => None,
    }
}

fn write_context_file(content: &[u8], plugin_name: &str, runtime_dir: &Path) -> Option<PathBuf> {
    let path = runtime_dir.join(format!("pre-launch-{}.md", plugin_name));
    fs::write(&path, content).ok()?;
    Some(path)
}

fn apply_env_lines(output: &[u8], plugin_name: &str) {
    let Ok(text) = std::str::from_utf8(output) else { return };
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let (key, value) = (key.trim(), value.trim());
            if !key.is_empty() {
                unsafe { std::env::set_var(key, value) };
                tracing::debug!("plugin '{plugin_name}' set env {key}");
            }
        }
    }
}

fn cache_path(plugin_name: &str) -> PathBuf {
    plugin::user_config_dir()
        .join("pre-launch-cache")
        .join(format!("{plugin_name}.out"))
}

fn read_if_fresh(path: &Path, ttl_secs: u64) -> Option<Vec<u8>> {
    let age = fs::metadata(path)
        .ok()?
        .modified()
        .ok()
        .and_then(|m| std::time::SystemTime::now().duration_since(m).ok())?;
    if age.as_secs() <= ttl_secs {
        fs::read(path).ok()
    } else {
        None
    }
}

fn persist_cache(plugin_name: &str, content: &[u8]) {
    let path = cache_path(plugin_name);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, content);
}

fn expand_tilde(raw: &str) -> PathBuf {
    if raw.starts_with("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
            return home.join(&raw[2..]);
        }
    }
    PathBuf::from(raw)
}
