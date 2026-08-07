//! Image generation: HTML templates via Chrome headless, or AI (DALL-E 3).
//!
//! Pipeline:
//!   template — HTML template → minijinja vars → Chrome headless → PNG/JPEG/WEBP
//!   ai       — text prompt → OpenAI DALL-E 3 API (via curl) → PNG

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::data_paths;

// ── embedded built-ins ────────────────────────────────────────────────────────

const BUILTIN_IMAGE_RULES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_image_rules.rs"));

const BUILTIN_IMAGE_TEMPLATES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_image_templates.rs"));

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
}

impl ImageFormat {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "webp" => Ok(Self::Webp),
            other => bail!("unknown image format: {other}. Use: png, jpeg, webp"),
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
        }
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageBackend {
    Template,
    Ai,
}

impl ImageBackend {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "template" | "html" => Ok(Self::Template),
            "ai" | "dalle" | "dall-e" | "openai" => Ok(Self::Ai),
            other => bail!("unknown backend: {other}. Use: template, ai"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Ai => "ai",
        }
    }
}

impl std::fmt::Display for ImageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ImageRule {
    pub format: String,
    pub renderer: String,
    pub template: Option<String>,
    pub width: u32,
    pub height: u32,
    pub quality: u32,
}

impl ImageRule {
    fn default_for(format: &ImageFormat) -> Self {
        Self {
            format: format.as_str().to_string(),
            renderer: "chrome".to_string(),
            template: Some("notice".to_string()),
            width: 1200,
            height: 630,
            quality: 90,
        }
    }
}

/// Metadata from the YAML front matter block in an HTML image template.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ImageTemplateMeta {
    pub name: String,
    pub description: String,
    pub variables: Vec<String>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateSource {
    Builtin,
    User(PathBuf),
    Workspace(PathBuf),
}

impl std::fmt::Display for TemplateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::User(p) => write!(f, "user:{}", p.display()),
            Self::Workspace(p) => write!(f, "workspace:{}", p.display()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageGenerateRequest {
    pub title: String,
    /// Text content stored as .txt alongside the image; used as template input or AI prompt.
    pub text_content: String,
    pub format: ImageFormat,
    pub backend: ImageBackend,
    pub template: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub vars: HashMap<String, String>,
    pub output: PathBuf,
    pub workspace: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
    /// Pre-allocated ID (e.g. IMG-000001). If set, the index uses it directly instead of
    /// calling next_id(). Set by the CLI so the ID can appear in the output filename.
    pub id: Option<String>,
    pub skip_index: bool,
}

pub struct ImageResult {
    pub output: PathBuf,
    pub source: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    pub id: String,
    pub title: String,
    pub format: String,
    pub backend: String,
    pub template: Option<String>,
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub workspace: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    pub vars: HashMap<String, String>,
    pub created_at: u64,
    pub updated_at: u64,
}

// ── generate ──────────────────────────────────────────────────────────────────

pub fn generate(req: &ImageGenerateRequest) -> Result<ImageResult> {
    if let Some(parent) = req.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output dir {}", parent.display()))?;
    }

    // Always persist the source text alongside the image.
    let source_path = req.output.with_extension("txt");
    if let Err(e) = fs::write(&source_path, &req.text_content) {
        eprintln!("[orbit image] warn: could not write source file: {e}");
    }

    match req.backend {
        ImageBackend::Template => generate_from_template(req)?,
        ImageBackend::Ai => generate_with_ai(req)?,
    }

    let bytes = fs::metadata(&req.output).map(|m| m.len()).unwrap_or(0);

    if !req.skip_index {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let ws = req.workspace.clone().unwrap_or_default();
        let id = req.id.clone().unwrap_or_else(|| next_id(&ws));
        let entry = ImageEntry {
            id,
            title: req.title.clone(),
            format: req.format.as_str().to_string(),
            backend: req.backend.as_str().to_string(),
            template: req.template.clone(),
            source_path: source_path.clone(),
            output_path: req.output.clone(),
            workspace: ws.clone(),
            tenant: req.tenant.clone().unwrap_or_default(),
            project: req.project.clone().unwrap_or_default(),
            repository: req.repository.clone().unwrap_or_default(),
            vars: req.vars.clone(),
            created_at: now,
            updated_at: now,
        };
        if let Err(e) = save_entry(&ws, &entry) {
            eprintln!("[orbit image] warn: could not save index entry: {e}");
        }
    }

    Ok(ImageResult {
        output: req.output.clone(),
        source: source_path,
        bytes,
    })
}

// ── template backend ──────────────────────────────────────────────────────────

fn generate_from_template(req: &ImageGenerateRequest) -> Result<()> {
    let rule = load_image_rule(&req.format);
    let width = req.width.unwrap_or(rule.width).max(1);
    let height = req.height.unwrap_or(rule.height).max(1);

    let rendered = render_template(req, &rule, width, height)?;

    let tmp_dir = std::env::temp_dir().join("orbit-image");
    fs::create_dir_all(&tmp_dir)?;
    let tmp_html = tmp_dir.join(format!("orbit-img-{}.html", random_suffix()));
    fs::write(&tmp_html, &rendered)
        .with_context(|| format!("cannot write temp HTML {}", tmp_html.display()))?;

    let result = chrome_screenshot(&tmp_html, &req.output, width, height);
    let _ = fs::remove_file(&tmp_html);
    result
}

fn render_template(
    req: &ImageGenerateRequest,
    rule: &ImageRule,
    width: u32,
    height: u32,
) -> Result<String> {
    let tpl_name = req
        .template
        .clone()
        .or_else(|| rule.template.clone())
        .unwrap_or_else(|| "notice".to_string());

    let (_source, raw) = resolve_template(&tpl_name)?;
    let body = strip_front_matter(&raw);

    let scope = [
        req.tenant.as_deref().unwrap_or(""),
        req.project.as_deref().unwrap_or(""),
        req.repository.as_deref().unwrap_or(""),
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .copied()
    .collect::<Vec<_>>()
    .join("/");

    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert("title".to_string(), req.title.clone());
    vars.insert("description".to_string(), req.text_content.clone());
    vars.insert("text_content".to_string(), req.text_content.clone());
    vars.insert("width".to_string(), width.to_string());
    vars.insert("height".to_string(), height.to_string());
    vars.insert(
        "orbit_workspace".to_string(),
        req.workspace.clone().unwrap_or_default(),
    );
    vars.insert("orbit_scope".to_string(), scope);

    // User vars override auto vars.
    vars.extend(req.vars.clone());

    let mut env = minijinja::Environment::new();
    env.add_template("tpl", body)
        .context("invalid image template syntax")?;
    env.get_template("tpl")
        .unwrap()
        .render(vars)
        .context("failed to render image template")
}

fn strip_front_matter(html: &str) -> &str {
    let trimmed = html.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<!-- ---")
        && let Some(idx) = rest.find("--- -->")
    {
        let after = &rest[idx + "--- -->".len()..];
        let offset = html.len() - after.len();
        return &html[offset..];
    }
    html
}

fn chrome_screenshot(html: &Path, output: &Path, width: u32, height: u32) -> Result<()> {
    let chrome = find_chrome()?;
    let abs = html.canonicalize().unwrap_or_else(|_| html.to_path_buf());

    let status = Command::new(&chrome)
        .args([
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            "--disable-software-rasterizer",
            "--hide-scrollbars",
            &format!("--window-size={width},{height}"),
            &format!("--screenshot={}", output.display()),
            &format!("file://{}", abs.display()),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("failed to run {chrome}"))?;

    if !status.success() {
        bail!("chrome headless exited with status: {status}");
    }

    if !output.exists() {
        bail!(
            "chrome ran but output file was not created: {}",
            output.display()
        );
    }

    Ok(())
}

fn find_chrome() -> Result<String> {
    for candidate in [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
        "google-chrome",
        "chromium",
        "chromium-browser",
    ] {
        let ok = Command::new(candidate)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            return Ok(candidate.to_string());
        }
    }
    bail!(
        "no Chrome/Chromium found. Install google-chrome or chromium to use the template backend."
    )
}

// ── AI backend ────────────────────────────────────────────────────────────────

fn generate_with_ai(req: &ImageGenerateRequest) -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .or_else(|| crate::secrets::keychain_get("openai_api_key").ok())
        .context(
            "AI backend requires OPENAI_API_KEY.\n  \
             Set it with: export OPENAI_API_KEY=sk-...\n  \
             Or store permanently: orbit secret set openai_api_key",
        )?;

    let prompt = if req.text_content.is_empty() {
        req.title.clone()
    } else {
        format!("{}: {}", req.title, req.text_content)
    };

    let size = match (req.width.unwrap_or(1024), req.height.unwrap_or(1024)) {
        (w, h) if w > h => "1792x1024",
        (w, h) if h > w => "1024x1792",
        _ => "1024x1024",
    };

    let payload = serde_json::json!({
        "model": "dall-e-3",
        "prompt": prompt,
        "n": 1,
        "size": size,
        "response_format": "url",
    });
    let payload_str = serde_json::to_string(&payload)?;

    let api_out = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "https://api.openai.com/v1/images/generations",
            "-H",
            &format!("Authorization: Bearer {api_key}"),
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload_str,
        ])
        .output()
        .context("failed to run curl for OpenAI images API")?;

    if !api_out.status.success() {
        bail!("curl exited with status {}", api_out.status);
    }

    let resp: serde_json::Value =
        serde_json::from_slice(&api_out.stdout).context("failed to parse OpenAI API response")?;

    if let Some(err) = resp.get("error") {
        bail!(
            "OpenAI API error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        );
    }

    let url = resp["data"][0]["url"]
        .as_str()
        .context("no image URL in OpenAI API response")?;

    let dl_status = Command::new("curl")
        .args(["-s", "-L", "-o", &req.output.to_string_lossy(), url])
        .status()
        .context("failed to run curl to download image")?;

    if !dl_status.success() {
        bail!("curl failed to download image from OpenAI");
    }

    Ok(())
}

// ── rule loading ──────────────────────────────────────────────────────────────

fn load_image_rule(format: &ImageFormat) -> ImageRule {
    let fmt = format.as_str();

    if let Some(dir) = data_paths::workspace_image_rules_dir() {
        let p = dir.join(format!("{fmt}.yaml"));
        if let Ok(raw) = fs::read_to_string(&p)
            && let Ok(rule) = serde_yml::from_str::<ImageRule>(&raw)
        {
            return rule;
        }
    }

    let user_p = data_paths::image_rules_dir().join(format!("{fmt}.yaml"));
    if let Ok(raw) = fs::read_to_string(&user_p)
        && let Ok(rule) = serde_yml::from_str::<ImageRule>(&raw)
    {
        return rule;
    }

    for (name, content) in BUILTIN_IMAGE_RULES {
        if *name == fmt && let Ok(rule) = serde_yml::from_str::<ImageRule>(content) {
            return rule;
        }
    }

    ImageRule::default_for(format)
}

// ── template management ───────────────────────────────────────────────────────

pub fn list_templates() -> Vec<(TemplateSource, ImageTemplateMeta)> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();

    if let Some(dir) = data_paths::workspace_image_templates_dir() {
        append_templates_from_dir(&dir, &mut seen, &mut out, true);
    }

    let user_dir = data_paths::image_templates_dir();
    append_templates_from_dir(&user_dir, &mut seen, &mut out, false);

    for (name, content) in BUILTIN_IMAGE_TEMPLATES {
        if seen.insert(name.to_string()) {
            out.push((TemplateSource::Builtin, parse_template_meta(content, name)));
        }
    }

    out
}

fn append_templates_from_dir(
    dir: &Path,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<(TemplateSource, ImageTemplateMeta)>,
    is_workspace: bool,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
        .collect();
    paths.sort_by_key(|e| e.path());
    for entry in paths {
        let name = stem(&entry.path());
        if seen.insert(name.clone()) && let Ok(raw) = fs::read_to_string(entry.path()) {
            let src = if is_workspace {
                TemplateSource::Workspace(entry.path())
            } else {
                TemplateSource::User(entry.path())
            };
            out.push((src, parse_template_meta(&raw, &name)));
        }
    }
}

pub fn resolve_template(name: &str) -> Result<(String, String)> {
    if let Some(dir) = data_paths::workspace_image_templates_dir() {
        let p = dir.join(format!("{name}.html"));
        if p.exists() {
            let raw = fs::read_to_string(&p)
                .with_context(|| format!("cannot read workspace template {}", p.display()))?;
            return Ok((format!("workspace:{}", p.display()), raw));
        }
    }

    let user_p = data_paths::image_templates_dir().join(format!("{name}.html"));
    if user_p.exists() {
        let raw = fs::read_to_string(&user_p)
            .with_context(|| format!("cannot read user template {}", user_p.display()))?;
        return Ok((format!("user:{}", user_p.display()), raw));
    }

    for (tname, content) in BUILTIN_IMAGE_TEMPLATES {
        if *tname == name {
            return Ok(("builtin".to_string(), content.to_string()));
        }
    }

    bail!("image template '{name}' not found. Run `orbit image template list` to see available.")
}

pub fn add_user_template(path: &Path) -> Result<(String, ImageTemplateMeta)> {
    if path.extension().is_none_or(|e| e != "html") {
        bail!("template must be an .html file");
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let name = stem(path);
    let meta = parse_template_meta(&raw, &name);
    let dest_dir = data_paths::image_templates_dir();
    fs::create_dir_all(&dest_dir).context("cannot create image templates directory")?;
    let dest = dest_dir.join(format!("{name}.html"));
    fs::copy(path, &dest).with_context(|| format!("cannot copy template to {}", dest.display()))?;
    Ok((name, meta))
}

pub fn remove_user_template(name: &str) -> Result<()> {
    let p = data_paths::image_templates_dir().join(format!("{name}.html"));
    if !p.exists() {
        bail!("user image template not found: {name}");
    }
    fs::remove_file(&p).with_context(|| format!("cannot remove {}", p.display()))
}

fn parse_template_meta(html: &str, fallback: &str) -> ImageTemplateMeta {
    let trimmed = html.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<!-- ---")
        && let Some(end) = rest.find("--- -->")
    {
        let yaml = rest[..end].trim();
        if let Ok(meta) = serde_yml::from_str::<ImageTemplateMeta>(yaml) {
            return meta;
        }
    }
    ImageTemplateMeta {
        name: fallback.to_string(),
        ..Default::default()
    }
}

// ── NDJSON index ──────────────────────────────────────────────────────────────

pub fn save_entry(workspace_name: &str, entry: &ImageEntry) -> Result<()> {
    let path = data_paths::images_index_path_for(Some(workspace_name));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("cannot create images index directory")?;
    }
    let line = serde_json::to_string(entry).context("cannot serialise image entry")?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("cannot open index {}", path.display()))?;
    writeln!(f, "{line}").context("cannot write index entry")
}

pub fn load_all_entries(workspace_name: &str) -> Vec<ImageEntry> {
    read_entries(&data_paths::images_index_path_for(Some(workspace_name)))
}

pub fn load_all_entries_global() -> Vec<ImageEntry> {
    data_paths::all_images_index_paths()
        .into_iter()
        .flat_map(|p| read_entries(&p))
        .collect()
}

fn read_entries(path: &Path) -> Vec<ImageEntry> {
    let Ok(content) = fs::read_to_string(path) else {
        return vec![];
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn find_entry(query: &str) -> Option<(String, ImageEntry)> {
    let ws = std::env::var("AI_WORKSPACE_ROOT")
        .ok()
        .and_then(|p| {
            PathBuf::from(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_default();

    let search = if !ws.is_empty() {
        load_all_entries(&ws)
    } else {
        load_all_entries_global()
    };

    for e in &search {
        if e.id == query
            || e.output_path.to_string_lossy() == query
            || e.source_path.to_string_lossy() == query
        {
            return Some((ws.clone(), e.clone()));
        }
    }

    for e in &load_all_entries_global() {
        if e.id == query || e.output_path.to_string_lossy() == query {
            return Some((e.workspace.clone(), e.clone()));
        }
    }

    None
}

pub fn update_stored_entry(workspace_name: &str, id: &str, updated: ImageEntry) -> Result<()> {
    let path = data_paths::images_index_path_for(Some(workspace_name));
    let new_content: String = read_entries(&path)
        .into_iter()
        .map(|e| if e.id == id { updated.clone() } else { e })
        .map(|e| format!("{}\n", serde_json::to_string(&e).unwrap_or_default()))
        .collect();
    fs::write(&path, new_content)
        .with_context(|| format!("cannot rewrite index {}", path.display()))
}

pub fn next_id(workspace_name: &str) -> String {
    format!("IMG-{:06}", load_all_entries(workspace_name).len() + 1)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn random_suffix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
        ^ (std::process::id() as u64).wrapping_mul(0x9e3779b97f4a7c15)
}
