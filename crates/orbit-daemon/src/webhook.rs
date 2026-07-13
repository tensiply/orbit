use orbit_core::user_config::UserConfig;
use tracing::warn;

/// Fire the configured webhook for a plan terminal event.
/// Reads config from disk each time so users don't need to restart the daemon.
/// Runs in a spawned thread — all errors are logged and swallowed.
pub fn maybe_fire(plan_id: &str, intent: &str, is_failure: bool) {
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
    if nc.webhook.url.is_empty() {
        return;
    }

    let url = nc.webhook.url.clone();
    let secret = nc.webhook.secret.clone();
    let event = if is_failure {
        "plan_failed"
    } else {
        "plan_completed"
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let payload = serde_json::json!({
        "event": event,
        "plan_id": plan_id,
        "intent": intent,
        "timestamp": now,
    });

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("webhook: failed to build HTTP client: {e}");
            return;
        }
    };

    let body = serde_json::to_string(&payload).unwrap_or_default();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body);

    if !secret.is_empty() {
        req = req.header("Authorization", format!("Bearer {secret}"));
    }

    match req.send() {
        Ok(resp) => {
            if !resp.status().is_success() {
                warn!("webhook: POST {url} returned {}", resp.status());
            }
        }
        Err(e) => {
            warn!("webhook: POST {url} failed: {e}");
        }
    }
}
