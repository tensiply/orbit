use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::data_paths;

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Format a Unix timestamp as `YYYY-MM-DD HH:MM` (UTC).
pub fn format_ts(ts: u64) -> String {
    let m = (ts / 60) % 60;
    let h = (ts / 3600) % 24;
    let days = ts / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, m)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d)
}

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub ts: u64,
    pub scope: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl ActivityEntry {
    pub fn new(scope: String, summary: String) -> Self {
        Self {
            ts: now_secs(),
            scope,
            summary,
            session_id: None,
        }
    }

    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

// ── scope key ─────────────────────────────────────────────────────────────────

/// Build a scope key from tenant/project/repository parts, e.g. `AIDEV/AI-ECOSYSTEM/orbit`.
pub fn scope_key(tenant: Option<&str>, project: Option<&str>, repository: Option<&str>) -> String {
    [tenant, project, repository]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

// ── storage ───────────────────────────────────────────────────────────────────

pub fn append(workspace: Option<&str>, entry: &ActivityEntry) -> Result<()> {
    let path = data_paths::activity_index_path_for(workspace);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}

/// Returns entries in reverse-chronological order (newest first).
pub fn list(
    workspace: Option<&str>,
    scope_filter: Option<&str>,
    session_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<ActivityEntry>> {
    let path = data_paths::activity_index_path_for(workspace);
    if !path.exists() {
        return Ok(vec![]);
    }
    let reader = BufReader::new(fs::File::open(&path)?);
    let mut entries: Vec<ActivityEntry> = reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .filter(|e: &ActivityEntry| {
            scope_filter.is_none_or(|s| e.scope.to_lowercase().contains(&s.to_lowercase()))
        })
        .filter(|e: &ActivityEntry| {
            session_filter.is_none_or(|s| e.session_id.as_deref() == Some(s))
        })
        .collect();
    entries.reverse();
    entries.truncate(limit);
    Ok(entries)
}

pub fn session_exists(workspace: Option<&str>, session_id: &str) -> bool {
    list(workspace, None, Some(session_id), 1)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// Format entries as a markdown block suitable for injection into context.
pub fn format_for_context(entries: &[ActivityEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut out = String::from("## Actividad reciente\n\n");
    for e in entries {
        out.push_str(&format!(
            "- **{}** `{}` — {}\n",
            format_ts(e.ts),
            e.scope,
            e.summary.lines().next().unwrap_or(&e.summary),
        ));
    }
    out
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_ts_known_date() {
        // 2025-07-23 00:00:00 UTC = 1753228800
        let ts = 1753228800u64;
        let s = format_ts(ts);
        assert!(s.starts_with("2025-07-23"), "got: {s}");
    }

    #[test]
    fn scope_key_full() {
        assert_eq!(
            scope_key(Some("AIDEV"), Some("AI-ECOSYSTEM"), Some("orbit")),
            "AIDEV/AI-ECOSYSTEM/orbit"
        );
    }

    #[test]
    fn scope_key_partial() {
        assert_eq!(scope_key(Some("AIDEV"), None, None), "AIDEV");
        assert_eq!(scope_key(None, None, Some("orbit")), "orbit");
    }
}
