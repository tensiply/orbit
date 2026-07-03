use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

// ── org type ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JiraOrg {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub user: String,
}

// ── issue + task context ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    /// statusCategory.colorName: "blue-gray" | "yellow" | "green" | "medium-gray"
    pub status_color: String,
    pub priority: String,
    pub issue_type: String,
    pub board_id: String,
    pub board_name: String,
    pub org: String,
}

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub priority: String,
    pub issue_type: String,
    pub board_id: String,
    pub board_name: String,
    pub org: String,
}

impl From<JiraIssue> for TaskContext {
    fn from(issue: JiraIssue) -> Self {
        Self {
            key: issue.key,
            summary: issue.summary,
            status: issue.status,
            priority: issue.priority,
            issue_type: issue.issue_type,
            board_id: issue.board_id,
            board_name: issue.board_name,
            org: issue.org,
        }
    }
}

// ── paths ─────────────────────────────────────────────────────────────────────

/// Path to the manual orgs override file — local-only, never committed.
pub fn orgs_path() -> PathBuf {
    crate::plugin::user_config_dir().join("plugins/jira/orgs.toml")
}

/// Path to acli's Jira config. acli always uses $HOME/.config regardless of
/// XDG_CONFIG_HOME, so we resolve HOME directly to avoid workspace overrides.
fn acli_jira_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            directories::BaseDirs::new()
                .map(|b| b.home_dir().to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"))
        });
    home.join(".config/acli/jira_config.yaml")
}

// ── I/O ───────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OrgsFile {
    #[serde(default)]
    orgs: Vec<JiraOrg>,
    poll_interval_secs: Option<u64>,
}

/// Read orgs from acli's jira_config.yaml (authenticated profiles).
/// Each profile yields a JiraOrg with id = site-without-domain suffix,
/// url = https://{site}, user = email.
pub fn discover_orgs() -> Vec<JiraOrg> {
    let content = match fs::read_to_string(acli_jira_config_path()) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut orgs = Vec::new();
    let mut current_site: Option<String> = None;
    let mut current_email = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- site: ") {
            if let Some(site) = current_site.take() {
                orgs.push(org_from_acli_profile(site, current_email.clone()));
            }
            current_site = Some(rest.trim().to_string());
            current_email = String::new();
        } else if current_site.is_some() {
            if let Some(email) = trimmed.strip_prefix("email: ") {
                current_email = email.trim().to_string();
            }
        }
    }
    if let Some(site) = current_site {
        orgs.push(org_from_acli_profile(site, current_email));
    }

    orgs
}

fn org_from_acli_profile(site: String, email: String) -> JiraOrg {
    let id = site
        .strip_suffix(".atlassian.net")
        .unwrap_or(&site)
        .to_string();
    let url = format!("https://{site}");
    JiraOrg { id, url, user: email }
}

/// Load orgs: manual config (orgs.toml) takes precedence; falls back to
/// auto-discovery from acli's authenticated profiles.
pub fn load_orgs() -> Vec<JiraOrg> {
    let path = orgs_path();
    let content = fs::read_to_string(&path).unwrap_or_default();
    let manual = toml::from_str::<OrgsFile>(&content)
        .map(|f| f.orgs)
        .unwrap_or_default();

    if !manual.is_empty() {
        return manual;
    }

    discover_orgs()
}

/// Poll interval configured in orgs.toml; defaults to 300s (5 minutes).
pub fn poll_interval_secs() -> u64 {
    let content = fs::read_to_string(orgs_path()).unwrap_or_default();
    toml::from_str::<OrgsFile>(&content)
        .ok()
        .and_then(|f| f.poll_interval_secs)
        .unwrap_or(300)
}

// ── cache helpers ─────────────────────────────────────────────────────────────

pub fn issues_cache_path() -> PathBuf {
    crate::plugin::user_config_dir().join("plugins/jira/cache.json")
}

pub fn write_issues_cache(issues: &[JiraIssue]) {
    let path = issues_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(issues) {
        let _ = fs::write(&path, json);
    }
}

pub fn read_issues_cache() -> Vec<JiraIssue> {
    let content = fs::read_to_string(issues_cache_path()).unwrap_or_default();
    serde_json::from_str::<Vec<JiraIssue>>(&content).unwrap_or_default()
}

pub fn cache_mtime() -> Option<std::time::SystemTime> {
    fs::metadata(issues_cache_path()).ok()?.modified().ok()
}

// ── fetch issues via acli (per-org auth-switch sweep) ─────────────────────────

/// Returns the site currently active in acli (e.g. "mysite.atlassian.net").
fn current_auth_site() -> Option<String> {
    let out = Command::new("acli")
        .args(["jira", "auth", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = std::str::from_utf8(&out.stdout).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Site:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Switch acli to the given site (strip scheme from URL).
fn auth_switch(site: &str) {
    let host = site
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let _ = Command::new("acli")
        .args(["jira", "auth", "switch", "--site", host])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Fetch all issues assigned to the current user across every configured org.
/// For multiple orgs, switches acli auth context per org and restores it after.
pub fn fetch_issues(orgs: &[JiraOrg]) -> Vec<JiraIssue> {
    if orgs.is_empty() {
        return vec![];
    }

    let multi = orgs.len() > 1;
    let original_site = if multi { current_auth_site() } else { None };

    let mut all = Vec::new();

    for org in orgs {
        if multi {
            auth_switch(&org.url);
        }

        let output = Command::new("acli")
            .args([
                "jira",
                "workitem",
                "search",
                "--jql",
                "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC",
                "--json",
            ])
            .output();

        let Ok(out) = output else { continue };
        if !out.status.success() {
            continue;
        }

        let Ok(text) = std::str::from_utf8(&out.stdout) else {
            continue;
        };
        let Ok(val) = serde_json::from_str::<serde_json::Value>(text) else {
            continue;
        };

        let empty = vec![];
        let issues_arr = val
            .as_array()
            .or_else(|| val.get("issues").and_then(|v| v.as_array()))
            .or_else(|| val.get("workItems").and_then(|v| v.as_array()))
            .unwrap_or(&empty);

        for item in issues_arr {
            let key = str_field(item, "key");
            if key.is_empty() {
                continue;
            }

            // acli: issue data lives under `fields`; fall back to item itself
            let fields = item.get("fields").unwrap_or(item);

            let status = nested_name(fields, "status")
                .unwrap_or_else(|| str_field(fields, "status"));

            // statusCategory.colorName: "blue-gray" | "yellow" | "green" | "medium-gray"
            let status_color = fields
                .get("status")
                .and_then(|s| s.get("statusCategory"))
                .and_then(|sc| sc.get("colorName"))
                .and_then(|v| v.as_str())
                .unwrap_or("blue-gray")
                .to_string();

            let priority = nested_name(fields, "priority")
                .unwrap_or_else(|| str_field(fields, "priority"));

            let issue_type = nested_name(fields, "issuetype")
                .unwrap_or_default();

            let board_id = key.split('-').next().unwrap_or("").to_string();

            all.push(JiraIssue {
                key,
                summary: str_field(fields, "summary"),
                status,
                status_color,
                priority,
                issue_type,
                board_id,
                board_name: String::new(),
                org: org.id.clone(),
            });
        }
    }

    // Restore original auth context
    if multi {
        if let Some(site) = original_site {
            auth_switch(&site);
        }
    }

    all
}

fn str_field(val: &serde_json::Value, key: &str) -> String {
    val.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Extract the "name" sub-field from a nested object (e.g. status.name).
fn nested_name(val: &serde_json::Value, key: &str) -> Option<String> {
    val.get(key)?.get("name")?.as_str().map(String::from)
}

// ── render task context as engine instructions ────────────────────────────────

pub fn render_task_instructions(task: &TaskContext) -> String {
    format!(
        "## Active Task\n\n**{}** — {}\nStatus: {} · Priority: {} · Board: {}\n\nWork on this issue in the current session.\n\n\
Available Jira commands (run from terminal):\n\
- `orbit jira comment {key} \"your comment\"` — add a comment to this issue\n\
- `orbit jira describe {key} \"new description\"` — update the issue description\n",
        task.key, task.summary, task.status, task.priority, task.board_name,
        key = task.key,
    )
}

/// Render full issue detail (description + comments) as Markdown for context injection.
pub fn render_task_detail_instructions(detail: &JiraIssueDetail) -> String {
    let mut md = format!(
        "## Active Task: {} — {}\n\n",
        detail.key, detail.summary
    );

    md.push_str(&format!(
        "**Status:** {} · **Priority:** {} · **Type:** {}\n",
        detail.status, detail.priority, detail.issue_type
    ));
    md.push_str(&format!(
        "**Assignee:** {} · **Reporter:** {}\n",
        detail.assignee, detail.reporter
    ));
    if !detail.sprint.is_empty() {
        md.push_str(&format!("**Sprint:** {}\n", detail.sprint));
    }
    if let Some(pts) = detail.story_points {
        md.push_str(&format!("**Story Points:** {pts}\n"));
    }
    if !detail.due_date.is_empty() {
        md.push_str(&format!("**Due:** {}\n", detail.due_date));
    }

    if !detail.description.is_empty() {
        md.push_str("\n### Description\n\n");
        md.push_str(&detail.description);
    }

    if !detail.comments.is_empty() {
        md.push_str("\n### Recent Comments\n\n");
        let skip = detail.comments.len().saturating_sub(5);
        for comment in &detail.comments[skip..] {
            md.push_str(&format!(
                "**{}** ({})\n{}\n\n",
                comment.author, comment.created, comment.body
            ));
        }
    }

    let key = &detail.key;
    md.push_str(&format!(
        "\nAvailable Jira commands:\n\
        - `orbit jira comment {key} \"your comment\"` — add a comment\n\
        - `orbit jira describe {key} \"new description\"` — update description\n"
    ));

    md
}

// ── write operations ──────────────────────────────────────────────────────────

pub fn add_comment(key: &str, body: &str) -> Result<(), String> {
    let out = Command::new("acli")
        .args(["jira", "workitem", "comment", "create", "--key", key, "--body", body])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("acli error: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = std::str::from_utf8(&out.stderr).unwrap_or("").trim().to_string();
        Err(if msg.is_empty() { format!("acli returned non-zero for comment on {key}") } else { msg })
    }
}

pub fn update_description(key: &str, body: &str) -> Result<(), String> {
    let out = Command::new("acli")
        .args(["jira", "workitem", "edit", "--key", key, "--description", body, "--yes"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("acli error: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = std::str::from_utf8(&out.stderr).unwrap_or("").trim().to_string();
        Err(if msg.is_empty() { format!("acli returned non-zero for description on {key}") } else { msg })
    }
}

// ── full issue detail ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct JiraComment {
    pub author: String,
    pub created: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct JiraIssueDetail {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub status_color: String,
    pub priority: String,
    pub issue_type: String,
    pub assignee: String,
    pub reporter: String,
    pub created: String,
    pub updated: String,
    pub due_date: String,
    pub sprint: String,
    pub story_points: Option<f64>,
    pub description: String,
    /// Raw ADF JSON — preserved for TUI rendering and append operations.
    pub description_adf: Option<serde_json::Value>,
    pub comments: Vec<JiraComment>,
}

/// Fetch full details for a single issue using `acli jira workitem view`.
pub fn fetch_issue_detail(key: &str) -> Result<JiraIssueDetail, String> {
    let out = Command::new("acli")
        .args([
            "jira", "workitem", "view", key,
            "--fields", "*all",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("acli error: {e}"))?;

    if !out.status.success() {
        let msg = std::str::from_utf8(&out.stderr).unwrap_or("").trim().to_string();
        return Err(if msg.is_empty() { format!("acli returned non-zero for {key}") } else { msg });
    }

    let text = std::str::from_utf8(&out.stdout).map_err(|e| e.to_string())?;
    let val: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let fields = val.get("fields").unwrap_or(&val);

    let status = nested_name(fields, "status").unwrap_or_else(|| str_field(fields, "status"));
    let status_color = fields
        .get("status").and_then(|s| s.get("statusCategory"))
        .and_then(|sc| sc.get("colorName")).and_then(|v| v.as_str())
        .unwrap_or("blue-gray").to_string();

    let sprint = fields
        .get("customfield_10010").and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|s| s.get("name")).and_then(|v| v.as_str())
        .unwrap_or("").to_string();

    let story_points = fields
        .get("customfield_10014")
        .and_then(|v| v.as_f64());

    let description_adf = fields.get("description").cloned();
    let description = description_adf.as_ref()
        .map(|d| extract_adf_text(d))
        .unwrap_or_default();

    let comments = fields
        .get("comment").and_then(|c| c.get("comments")).and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().map(|c| {
                let author = c.get("author").and_then(|a| a.get("displayName"))
                    .and_then(|v| v.as_str()).unwrap_or("?").to_string();
                let created = c.get("created").and_then(|v| v.as_str())
                    .map(fmt_date).unwrap_or_default();
                let body = c.get("body").map(|b| extract_adf_text(b)).unwrap_or_default();
                JiraComment { author, created, body }
            }).collect()
        })
        .unwrap_or_default();

    Ok(JiraIssueDetail {
        key: str_field(&val, "key"),
        summary: str_field(fields, "summary"),
        status,
        status_color,
        priority: nested_name(fields, "priority").unwrap_or_default(),
        issue_type: nested_name(fields, "issuetype").unwrap_or_default(),
        assignee: fields.get("assignee").and_then(|a| a.get("displayName"))
            .and_then(|v| v.as_str()).unwrap_or("Unassigned").to_string(),
        reporter: fields.get("reporter").and_then(|r| r.get("displayName"))
            .and_then(|v| v.as_str()).unwrap_or("").to_string(),
        created: fields.get("created").and_then(|v| v.as_str()).map(fmt_date).unwrap_or_default(),
        updated: fields.get("updated").and_then(|v| v.as_str()).map(fmt_date).unwrap_or_default(),
        due_date: fields.get("duedate").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        sprint,
        story_points,
        description,
        description_adf,
        comments,
    })
}

/// Append `text` as a new paragraph to an issue's existing description,
/// preserving all existing content (tables, images, etc.).
pub fn append_description(key: &str, text: &str) -> Result<(), String> {
    // Fetch current description ADF
    let out = Command::new("acli")
        .args(["jira", "workitem", "view", key, "--fields", "description", "--json"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).output()
        .map_err(|e| format!("acli error: {e}"))?;

    let raw = std::str::from_utf8(&out.stdout).map_err(|e| e.to_string())?;
    let val: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let fields = val.get("fields").unwrap_or(&val);

    let mut doc = fields.get("description").cloned()
        .unwrap_or_else(|| serde_json::json!({"type": "doc", "version": 1, "content": []}));

    // Ensure doc wrapper
    if doc.get("type").and_then(|t| t.as_str()) != Some("doc") {
        doc = serde_json::json!({"type": "doc", "version": 1, "content": [doc]});
    }
    if doc.get("content").is_none() {
        doc["content"] = serde_json::json!([]);
    }

    let content = doc["content"].as_array_mut().ok_or("invalid ADF structure")?;
    // Separator only if there's existing content
    if !content.is_empty() {
        content.push(serde_json::json!({"type": "rule"}));
    }
    content.push(serde_json::json!({
        "type": "paragraph",
        "content": [{"type": "text", "text": text}]
    }));

    let tmp_path = std::env::temp_dir().join(format!("orbit-adf-{key}.json"));
    fs::write(&tmp_path, serde_json::to_string(&doc).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    let result = Command::new("acli")
        .args(["jira", "workitem", "edit", "--key", key,
               "--description-file", tmp_path.to_str().unwrap_or(""), "--yes"])
        .stdout(Stdio::null()).stderr(Stdio::piped()).output()
        .map_err(|e| format!("acli error: {e}"));

    let _ = fs::remove_file(&tmp_path);

    let out = result?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = std::str::from_utf8(&out.stderr).unwrap_or("").trim().to_string();
        Err(if msg.is_empty() { format!("acli returned non-zero for description on {key}") } else { msg })
    }
}

/// Recursively extract plain text from Atlassian Document Format (ADF) or plain string.
fn extract_adf_text(val: &serde_json::Value) -> String {
    if let Some(s) = val.as_str() {
        return s.to_string();
    }
    if val.get("type").and_then(|t| t.as_str()) == Some("text") {
        return val.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
    }
    if let Some(content) = val.get("content").and_then(|c| c.as_array()) {
        let parts: Vec<String> = content.iter().map(extract_adf_text).collect();
        let joined = parts.join("");
        // Add newline after block-level nodes
        let node_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if matches!(node_type, "paragraph" | "heading" | "listItem" | "bulletList" | "orderedList") {
            return format!("{joined}\n");
        }
        return joined;
    }
    String::new()
}

/// Format an ISO datetime to "YYYY-MM-DD HH:MM".
fn fmt_date(s: &str) -> String {
    // "2026-07-02T08:48:24.116-0600" → "2026-07-02 08:48"
    if s.len() >= 16 {
        format!("{} {}", &s[..10], &s[11..16])
    } else {
        s.to_string()
    }
}
