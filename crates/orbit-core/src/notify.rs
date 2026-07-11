use crate::user_config::UserConfig;
use serde::{Deserialize, Serialize};
use std::process::Command;

// ── config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationsConfig {
    pub enabled: bool,
    pub on_plan_complete: bool,
    pub on_plan_failed: bool,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_plan_complete: true,
            on_plan_failed: true,
        }
    }
}

// ── public API ────────────────────────────────────────────────────────────────

/// Fire a desktop notification if the user's config allows it for this outcome.
///
/// `is_failure` selects the `on_plan_failed` gate; otherwise `on_plan_complete`.
pub fn maybe_send(title: &str, body: &str, is_failure: bool) {
    let cfg = UserConfig::load();
    let nc = &cfg.notifications;
    if !nc.enabled {
        return;
    }
    if is_failure && !nc.on_plan_failed {
        return;
    }
    if !is_failure && !nc.on_plan_complete {
        return;
    }
    send_notification(title, body);
}

/// Send a desktop notification unconditionally, best-effort.
pub fn send_notification(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send")
            .args([title, body])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            body.replace('\\', "\\\\").replace('"', "\\\""),
            title.replace('\\', "\\\\").replace('"', "\\\""),
        );
        let _ = Command::new("osascript")
            .args(["-e", &script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (title, body);
    }
}

/// Returns true if the platform notification backend appears to be available.
pub fn backend_available() -> bool {
    #[cfg(target_os = "linux")]
    return Command::new("which")
        .arg("notify-send")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    #[cfg(target_os = "macos")]
    return true;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return false;
}
