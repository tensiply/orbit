use serde::{Deserialize, Serialize};

// ── config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineProvider {
    GithubActions,
    Jenkins,
}

impl std::fmt::Display for PipelineProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GithubActions => write!(f, "GitHub Actions"),
            Self::Jenkins => write!(f, "Jenkins"),
        }
    }
}

/// Pipeline entry stored under the `pipelines` key in a scope's `orbit.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub name: String,
    pub provider: PipelineProvider,
    /// GitHub Actions: `"owner/repo"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// GitHub Actions: branch filter (default "main")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// GitHub Actions: optional workflow name filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// Jenkins: base URL, e.g. `"https://jenkins.example.com"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Jenkins: job path, e.g. `"folder/job-name"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    /// Secret reference resolved via `orbit_core::secrets::resolve_scoped`.
    /// Accepted forms: `keychain://KEY`, `env://VAR`, `$VAR`, or a literal token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_secret: Option<String>,
}

// ── status ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Success,
    Failure,
    Running,
    Pending,
    Cancelled,
    Unknown,
}

impl RunStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Running => "running",
            Self::Pending => "pending",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Success => "✓",
            Self::Failure => "✗",
            Self::Running => "⟳",
            Self::Pending => "○",
            Self::Cancelled => "⊘",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub name: String,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    /// Provider-specific run ID (e.g. GitHub run number or Jenkins build number).
    pub id: String,
    /// Human-readable name (e.g. workflow name or Jenkins display name).
    pub run_name: String,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggered_by: Option<String>,
    /// Unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub steps: Vec<PipelineStep>,
}

/// Result of querying a single pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub config: PipelineConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<PipelineRun>,
    /// Unix timestamp when the status was fetched.
    pub fetched_at: u64,
    /// Populated when the provider API returned an error or auth failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Parse an RFC 3339 / ISO 8601 UTC string ("2024-01-01T12:00:00Z") to a
/// Unix timestamp in seconds. Returns `None` for any parse failure.
pub fn parse_rfc3339(s: &str) -> Option<u64> {
    let s = s
        .strip_suffix('Z')
        .or_else(|| s.strip_suffix("+00:00"))
        .unwrap_or(s);
    let (date, time) = s.split_once('T')?;
    let mut dp = date.split('-');
    let year: i64 = dp.next()?.parse().ok()?;
    let month: i64 = dp.next()?.parse().ok()?;
    let day: i64 = dp.next()?.parse().ok()?;
    let mut tp = time.split(':');
    let hour: i64 = tp.next()?.parse().ok()?;
    let min: i64 = tp.next()?.parse().ok()?;
    // seconds field may have a fractional part
    let sec_str = tp.next()?;
    let sec: i64 = sec_str.split('.').next().and_then(|s| s.parse().ok())?;

    let days = days_since_epoch(year, month, day);
    let secs = days * 86400 + hour * 3600 + min * 60 + sec;
    if secs < 0 { None } else { Some(secs as u64) }
}

fn days_since_epoch(year: i64, month: i64, day: i64) -> i64 {
    const DAYS: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut total = 0i64;
    for y in 1970..year {
        total += if is_leap(y) { 366 } else { 365 };
    }
    for m in 1..month {
        total += DAYS[(m - 1) as usize];
        if m == 2 && is_leap(year) {
            total += 1;
        }
    }
    total + day - 1
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Map GitHub `status` + `conclusion` to `RunStatus`.
pub fn github_run_status(status: &str, conclusion: Option<&str>) -> RunStatus {
    match status {
        "queued" | "waiting" => RunStatus::Pending,
        "in_progress" => RunStatus::Running,
        "completed" => match conclusion {
            Some("success") => RunStatus::Success,
            Some("failure") | Some("timed_out") => RunStatus::Failure,
            Some("cancelled") | Some("skipped") => RunStatus::Cancelled,
            _ => RunStatus::Unknown,
        },
        _ => RunStatus::Unknown,
    }
}

/// Map GitHub job step `status` + `conclusion` to `RunStatus`.
pub fn github_step_status(status: &str, conclusion: Option<&str>) -> RunStatus {
    github_run_status(status, conclusion)
}

/// Map Jenkins `building` flag + `result` string to `RunStatus`.
pub fn jenkins_run_status(building: bool, result: Option<&str>) -> RunStatus {
    if building {
        return RunStatus::Running;
    }
    match result {
        Some("SUCCESS") => RunStatus::Success,
        Some("FAILURE") | Some("UNSTABLE") => RunStatus::Failure,
        Some("ABORTED") => RunStatus::Cancelled,
        Some("NOT_BUILT") => RunStatus::Pending,
        _ => RunStatus::Unknown,
    }
}
