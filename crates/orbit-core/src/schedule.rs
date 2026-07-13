use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleKind {
    /// Run once at a specific Unix timestamp.
    Once { at: u64 },
    /// Cron expression (5-field: min hour dom mon dow).
    Cron { expr: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledPlan {
    pub id: String,
    pub intent: String,
    pub schedule: ScheduleKind,
    /// Additional repo paths passed to the planner.
    #[serde(default)]
    pub repos: Vec<PathBuf>,
    /// Scope overrides (workspace/tenant/project/repository).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Unix timestamp of the next scheduled fire, or None for exhausted Once schedules.
    pub next_run: Option<u64>,
    /// Unix timestamp of the last fire, or None if never run.
    pub last_run: Option<u64>,
    /// Number of times this schedule has fired.
    pub run_count: u64,
    pub created_at: u64,
}

// ── storage ───────────────────────────────────────────────────────────────────

fn schedules_path() -> PathBuf {
    crate::data_paths::schedules_path_for(None)
}

pub fn load_all() -> Vec<ScheduledPlan> {
    let path = schedules_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_all(schedules: &[ScheduledPlan]) -> Result<()> {
    let path = schedules_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(schedules)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn find(id: &str) -> Option<ScheduledPlan> {
    load_all()
        .into_iter()
        .find(|s| s.id == id || s.id.starts_with(id))
}

pub fn upsert(schedule: ScheduledPlan) -> Result<()> {
    let mut all = load_all();
    all.retain(|s| s.id != schedule.id);
    all.push(schedule);
    save_all(&all)
}

pub fn delete(id: &str) -> Result<bool> {
    let mut all = load_all();
    let before = all.len();
    all.retain(|s| s.id != id && !s.id.starts_with(id));
    let removed = all.len() < before;
    if removed {
        save_all(&all)?;
    }
    Ok(removed)
}

// ── cron parsing ──────────────────────────────────────────────────────────────

/// Compute the next Unix timestamp (from `after_secs`) that matches a 5-field cron
/// expression: "min hour dom mon dow"  (all 0-based except dom/mon 1-based, dow 0=Sun).
///
/// Supports `*`, integer literals, and `*/step`.  Returns None if no match in
/// the next 4 years (effectively "never").
pub fn next_cron_after(expr: &str, after_secs: u64) -> Result<Option<u64>> {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        anyhow::bail!("cron expression must have exactly 5 fields: '{expr}'");
    }

    let mins = parse_field(fields[0], 0, 59).context("minute field")?;
    let hours = parse_field(fields[1], 0, 23).context("hour field")?;
    let doms = parse_field(fields[2], 1, 31).context("day-of-month field")?;
    let months = parse_field(fields[3], 1, 12).context("month field")?;
    let dows = parse_field(fields[4], 0, 6).context("day-of-week field")?;

    // Advance one minute past `after_secs` so we don't re-fire in the same minute.
    let start = after_secs + 60;
    let start_min = (start / 60) * 60;

    // Scan minute-by-minute for up to 2 years (~1M minutes).
    for offset in 0u64..(525_600 * 2) {
        let ts = start_min + offset * 60;
        let (y, mo, d, h, m, wd) = ts_to_ymdhm(ts);
        let _ = y;
        if !months.contains(&(mo as u8)) {
            continue;
        }
        if !doms.contains(&(d as u8)) {
            continue;
        }
        if !dows.contains(&(wd as u8)) {
            continue;
        }
        if !hours.contains(&(h as u8)) {
            continue;
        }
        if !mins.contains(&(m as u8)) {
            continue;
        }
        return Ok(Some(ts));
    }

    Ok(None)
}

fn parse_field(s: &str, min: u8, max: u8) -> Result<Vec<u8>> {
    if s == "*" {
        return Ok((min..=max).collect());
    }
    if let Some(step_str) = s.strip_prefix("*/") {
        let step: u8 = step_str.parse().context("step must be an integer")?;
        if step == 0 {
            anyhow::bail!("step cannot be zero");
        }
        return Ok((min..=max).step_by(step as usize).collect());
    }
    // Single value or comma list
    let mut values = Vec::new();
    for part in s.split(',') {
        let v: u8 = part
            .trim()
            .parse()
            .context("field value must be an integer")?;
        if v < min || v > max {
            anyhow::bail!("value {v} out of range {min}..={max}");
        }
        values.push(v);
    }
    Ok(values)
}

/// Decompose a Unix timestamp into (year, month 1-12, day 1-31, hour, minute, weekday 0=Sun).
fn ts_to_ymdhm(ts: u64) -> (u32, u32, u32, u32, u32, u32) {
    // Days since Unix epoch
    let total_mins = ts / 60;
    let minute = (total_mins % 60) as u32;
    let total_hours = total_mins / 60;
    let hour = (total_hours % 24) as u32;
    let total_days = (total_hours / 24) as u32;

    // Weekday: 1970-01-01 was a Thursday (4)
    let weekday = (total_days + 4) % 7;

    // Gregorian calendar
    let mut y = 1970u32;
    let mut remaining = total_days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let mut mo = 1u32;
    loop {
        let dim = days_in_month(y, mo);
        if remaining < dim {
            break;
        }
        remaining -= dim;
        mo += 1;
    }

    (y, mo, remaining + 1, hour, minute, weekday)
}

fn is_leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

// ── display helpers ───────────────────────────────────────────────────────────

pub fn format_schedule(s: &ScheduleKind) -> String {
    match s {
        ScheduleKind::Once { at } => format!("once at {}", format_ts(*at)),
        ScheduleKind::Cron { expr } => format!("cron  {expr}"),
    }
}

pub fn format_ts(ts: u64) -> String {
    let (y, mo, d, h, m, _) = ts_to_ymdhm(ts);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

// ── ID generation ─────────────────────────────────────────────────────────────

pub fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let pid = std::process::id();
    format!("sched_{secs}_{pid}")
}

pub fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
