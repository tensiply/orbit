use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::{
    pipeline::{
        PipelineConfig, PipelineProvider, PipelineRun, PipelineStatus, PipelineStep, RunStatus,
        github_run_status, github_step_status, jenkins_run_status, parse_rfc3339,
    },
    resolver, secrets,
    user_config::UserConfig,
};
use serde_json::Value;
use std::{fs, path::PathBuf};

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct PipelinesArgs {
    #[command(subcommand)]
    pub command: Option<PipelinesCommand>,

    /// Tenant (defaults to AI_TENANT env var)
    #[arg(long)]
    pub tenant: Option<String>,

    /// Project (defaults to AI_PROJECT env var)
    #[arg(long)]
    pub project: Option<String>,

    /// Repository (defaults to AI_REPOSITORY env var)
    #[arg(long)]
    pub repository: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PipelinesCommand {
    /// Fetch and display the status of all configured pipelines (default)
    Status,
    /// List configured pipelines without fetching their status
    List,
}

// ── entry point ───────────────────────────────────────────────────────────────

pub async fn run(args: PipelinesArgs) -> Result<()> {
    let (configs, workspace_slug) = collect_configs(&args)?;

    match args.command.unwrap_or(PipelinesCommand::Status) {
        PipelinesCommand::List => cmd_list(&configs),
        PipelinesCommand::Status => cmd_status(configs, workspace_slug).await,
    }
}

// ── config collection ─────────────────────────────────────────────────────────

fn collect_configs(args: &PipelinesArgs) -> Result<(Vec<PipelineConfig>, Option<String>)> {
    // Prefer the workspace detected from CWD (same logic as `orbit context show`)
    // so the correct ai_context_root is used regardless of what ai_root is set
    // in the static user config (which may point to a different workspace).
    let ai_root = resolver::resolve_from_cwd()
        .map(|s| s.ai_context_root)
        .unwrap_or_else(|_| UserConfig::load().ai_root_expanded());
    let slug = ai_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());

    let tenant = args
        .tenant
        .clone()
        .or_else(|| std::env::var("AI_TENANT").ok().filter(|s| !s.is_empty()));
    let project = args
        .project
        .clone()
        .or_else(|| std::env::var("AI_PROJECT").ok().filter(|s| !s.is_empty()));
    let repository = args.repository.clone().or_else(|| {
        std::env::var("AI_REPOSITORY")
            .ok()
            .filter(|s| !s.is_empty())
    });

    let paths = orbit_json_paths(
        &ai_root,
        tenant.as_deref(),
        project.as_deref(),
        repository.as_deref(),
    );
    let configs: Vec<PipelineConfig> = paths
        .iter()
        .flat_map(|p| read_pipelines_from_file(p))
        .collect();

    Ok((configs, slug))
}

fn orbit_json_paths(
    ai_root: &std::path::Path,
    tenant: Option<&str>,
    project: Option<&str>,
    repository: Option<&str>,
) -> Vec<PathBuf> {
    let mut paths = vec![ai_root.join("orbit.json")];
    if let Some(t) = tenant {
        paths.push(ai_root.join("tenants").join(t).join("orbit.json"));
        if let Some(p) = project {
            paths.push(
                ai_root
                    .join("tenants")
                    .join(t)
                    .join("projects")
                    .join(p)
                    .join("orbit.json"),
            );
            if let Some(r) = repository {
                paths.push(
                    ai_root
                        .join("tenants")
                        .join(t)
                        .join("projects")
                        .join(p)
                        .join("repositories")
                        .join(r)
                        .join("orbit.json"),
                );
            }
        }
    }
    paths
}

fn read_pipelines_from_file(path: &std::path::Path) -> Vec<PipelineConfig> {
    let Ok(text) = fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };
    let Some(arr) = val.get("pipelines").and_then(|v| v.as_array()) else {
        return vec![];
    };
    arr.iter()
        .filter_map(|v| serde_json::from_value::<PipelineConfig>(v.clone()).ok())
        .collect()
}

// ── list ──────────────────────────────────────────────────────────────────────

fn cmd_list(configs: &[PipelineConfig]) -> Result<()> {
    println!("pipelines\n");
    if configs.is_empty() {
        println!("  No pipelines configured for this scope.");
        println!();
        println!("  \x1b[2mAdd a pipeline to orbit.json at any scope level:\x1b[0m");
        println!(
            "  \x1b[2m  \"pipelines\": [{{ \"name\": \"CI\", \"provider\": \"github_actions\", \"repo\": \"owner/repo\" }}]\x1b[0m"
        );
        return Ok(());
    }
    for cfg in configs {
        println!(
            "  \x1b[1m{}\x1b[0m  \x1b[2m({})\x1b[0m",
            cfg.name, cfg.provider
        );
        match cfg.provider {
            PipelineProvider::GithubActions => {
                if let Some(repo) = &cfg.repo {
                    let branch = cfg.branch.as_deref().unwrap_or("main");
                    println!("    repo: {repo}  branch: {branch}");
                }
                if let Some(wf) = &cfg.workflow {
                    println!("    workflow: {wf}");
                }
            }
            PipelineProvider::Jenkins => {
                if let Some(url) = &cfg.url {
                    println!("    url: {url}");
                }
                if let Some(job) = &cfg.job {
                    println!("    job: {job}");
                }
            }
        }
        let auth = if cfg.token_secret.is_some() {
            "configured"
        } else {
            "\x1b[33mnot set\x1b[0m"
        };
        println!("    auth: {auth}");
        println!();
    }
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

async fn cmd_status(configs: Vec<PipelineConfig>, workspace_slug: Option<String>) -> Result<()> {
    println!("pipelines\n");
    if configs.is_empty() {
        println!("  No pipelines configured for this scope.");
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .user_agent("orbit-cli")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    for cfg in configs {
        let token = cfg
            .token_secret
            .as_ref()
            .map(|s| secrets::resolve_scoped(s, workspace_slug.as_deref()))
            .filter(|t| !t.is_empty());

        let result = fetch_status(&client, &cfg, token.as_deref()).await;
        print_status(&cfg, &result);
    }
    Ok(())
}

fn print_status(cfg: &PipelineConfig, result: &Result<PipelineStatus, String>) {
    match result {
        Err(e) => {
            println!(
                "  \x1b[31m✗\x1b[0m  \x1b[1m{}\x1b[0m  \x1b[2m{}\x1b[0m",
                cfg.name, cfg.provider
            );
            println!("       error: {e}");
        }
        Ok(ps) => {
            let Some(run) = &ps.latest_run else {
                println!(
                    "  \x1b[2m?\x1b[0m  \x1b[1m{}\x1b[0m  \x1b[2m{}\x1b[0m  no runs found",
                    cfg.name, cfg.provider
                );
                println!();
                return;
            };

            let (color, sym) = status_ansi(&run.status);
            println!(
                "  {color}{sym}\x1b[0m  \x1b[1m{}\x1b[0m  \x1b[2m{}\x1b[0m  {}",
                cfg.name,
                cfg.provider,
                run.status.label()
            );
            if let Some(branch) = &run.branch {
                println!("       branch: {branch}");
            }
            if let Some(msg) = &run.commit_message {
                let first_line = msg.lines().next().unwrap_or(msg);
                println!("       commit: {}", truncate(first_line, 72));
            }
            if let Some(by) = &run.triggered_by {
                println!("       by:     {by}");
            }
            if let Some(url) = &run.url {
                println!("       url:    {url}");
            }

            if !run.steps.is_empty() {
                println!("       steps:");
                for step in &run.steps {
                    let (sc, ss) = status_ansi(&step.status);
                    println!(
                        "         {sc}{ss}\x1b[0m  {}  \x1b[2m{}\x1b[0m",
                        step.name,
                        step.status.label()
                    );
                    if let Some(msg) = &step.message {
                        println!("              \x1b[31m{msg}\x1b[0m");
                    }
                }
            }
        }
    }
    println!();
}

fn status_ansi(s: &RunStatus) -> (&'static str, &'static str) {
    match s {
        RunStatus::Success => ("\x1b[32m", "✓"),
        RunStatus::Failure => ("\x1b[31m", "✗"),
        RunStatus::Running => ("\x1b[33m", "⟳"),
        RunStatus::Pending => ("\x1b[2m", "○"),
        RunStatus::Cancelled => ("\x1b[2m", "⊘"),
        RunStatus::Unknown => ("\x1b[2m", "?"),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

// ── fetcher dispatch ──────────────────────────────────────────────────────────

async fn fetch_status(
    client: &reqwest::Client,
    cfg: &PipelineConfig,
    token: Option<&str>,
) -> Result<PipelineStatus, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let result = match cfg.provider {
        PipelineProvider::GithubActions => fetch_github(client, cfg, token, now).await,
        PipelineProvider::Jenkins => fetch_jenkins(client, cfg, token, now).await,
    };

    match result {
        Ok(ps) => Ok(ps),
        Err(e) => Ok(PipelineStatus {
            config: cfg.clone(),
            latest_run: None,
            fetched_at: now,
            error: Some(e.to_string()),
        }),
    }
}

// ── GitHub Actions ────────────────────────────────────────────────────────────

async fn fetch_github(
    client: &reqwest::Client,
    cfg: &PipelineConfig,
    token: Option<&str>,
    now: u64,
) -> Result<PipelineStatus> {
    let repo = cfg
        .repo
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing 'repo' field"))?;
    let branch = cfg.branch.as_deref().unwrap_or("main");

    let mut url = format!(
        "https://api.github.com/repos/{}/actions/runs?branch={}&per_page=1",
        repo, branch
    );
    if let Some(wf) = &cfg.workflow {
        url.push_str(&format!("&event=push&workflow_name={}", wf));
    }

    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }

    let resp: Value = req.send().await?.error_for_status()?.json().await?;

    let runs = resp["workflow_runs"].as_array();
    let run_val = runs.and_then(|a| a.first());

    let latest_run = run_val.map(|r| {
        let status = r["status"].as_str().unwrap_or("unknown");
        let conclusion = r["conclusion"].as_str();
        let run_status = github_run_status(status, conclusion);

        let steps = fetch_github_steps_sync(r);

        PipelineRun {
            id: r["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
            run_name: r["name"].as_str().unwrap_or(&cfg.name).to_string(),
            status: run_status,
            branch: r["head_branch"].as_str().map(str::to_owned),
            commit_sha: r["head_sha"]
                .as_str()
                .map(|s| s[..8.min(s.len())].to_owned()),
            commit_message: r["head_commit"]["message"].as_str().map(str::to_owned),
            triggered_by: r["triggering_actor"]["login"].as_str().map(str::to_owned),
            started_at: r["created_at"].as_str().and_then(parse_rfc3339),
            completed_at: r["updated_at"].as_str().and_then(parse_rfc3339),
            url: r["html_url"].as_str().map(str::to_owned),
            steps,
        }
    });

    // Fetch job steps for the latest run (best-effort, non-fatal)
    let latest_run = if let Some(mut run) = latest_run {
        if let Some(run_id) = run_val.and_then(|r| r["id"].as_u64())
            && let Ok(steps) = fetch_github_jobs(client, repo, run_id, token).await
        {
            run.steps = steps;
        }
        Some(run)
    } else {
        None
    };

    Ok(PipelineStatus {
        config: cfg.clone(),
        latest_run,
        fetched_at: now,
        error: None,
    })
}

fn fetch_github_steps_sync(_run: &Value) -> Vec<PipelineStep> {
    vec![] // populated after the jobs call
}

async fn fetch_github_jobs(
    client: &reqwest::Client,
    repo: &str,
    run_id: u64,
    token: Option<&str>,
) -> Result<Vec<PipelineStep>> {
    let url = format!(
        "https://api.github.com/repos/{}/actions/runs/{}/jobs",
        repo, run_id
    );
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }

    let resp: Value = req.send().await?.error_for_status()?.json().await?;
    let Some(jobs) = resp["jobs"].as_array() else {
        return Ok(vec![]);
    };

    let mut steps = Vec::new();
    for job in jobs {
        let job_name = job["name"].as_str().unwrap_or("job").to_string();
        if let Some(job_steps) = job["steps"].as_array() {
            for s in job_steps {
                let name = s["name"].as_str().unwrap_or("step");
                steps.push(PipelineStep {
                    name: format!("{job_name} / {name}"),
                    status: github_step_status(
                        s["status"].as_str().unwrap_or("unknown"),
                        s["conclusion"].as_str(),
                    ),
                    started_at: s["started_at"].as_str().and_then(parse_rfc3339),
                    completed_at: s["completed_at"].as_str().and_then(parse_rfc3339),
                    message: None,
                });
            }
        }
    }
    Ok(steps)
}

// ── Jenkins ───────────────────────────────────────────────────────────────────

async fn fetch_jenkins(
    client: &reqwest::Client,
    cfg: &PipelineConfig,
    token: Option<&str>,
    now: u64,
) -> Result<PipelineStatus> {
    let base_url = cfg
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing 'url' field"))?;
    let job = cfg
        .job
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing 'job' field"))?;

    // Build job URL — replace slashes in job path with /job/ segments
    let job_segments: String = job
        .split('/')
        .map(|s| format!("job/{}", s))
        .collect::<Vec<_>>()
        .join("/");
    let api_url = format!(
        "{}/{}/lastBuild/api/json?tree=result,building,displayName,number,url,timestamp,duration",
        base_url.trim_end_matches('/'),
        job_segments
    );

    let mut req = client.get(&api_url);
    if let Some(t) = token {
        // Jenkins uses Basic auth: user:token (we treat the whole token as password, user = "")
        req = req.basic_auth("", Some(t));
    }

    let resp: Value = req.send().await?.error_for_status()?.json().await?;

    let building = resp["building"].as_bool().unwrap_or(false);
    let result_str = resp["result"].as_str();
    let run_status = jenkins_run_status(building, result_str);

    let timestamp_ms = resp["timestamp"].as_u64().unwrap_or(0);
    let duration_ms = resp["duration"].as_u64().unwrap_or(0);
    let started_at = if timestamp_ms > 0 {
        Some(timestamp_ms / 1000)
    } else {
        None
    };
    let completed_at = if timestamp_ms > 0 && duration_ms > 0 {
        Some((timestamp_ms + duration_ms) / 1000)
    } else {
        None
    };

    let build_number = resp["number"].as_u64().unwrap_or(0);
    let display_name = resp["displayName"]
        .as_str()
        .unwrap_or(&format!("#{build_number}"))
        .to_owned();

    Ok(PipelineStatus {
        config: cfg.clone(),
        latest_run: Some(PipelineRun {
            id: build_number.to_string(),
            run_name: display_name,
            status: run_status,
            branch: None,
            commit_sha: None,
            commit_message: None,
            triggered_by: None,
            started_at,
            completed_at,
            url: resp["url"].as_str().map(str::to_owned),
            steps: vec![],
        }),
        fetched_at: now,
        error: None,
    })
}
