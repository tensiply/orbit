//! SVG generation: minijinja templates or raw SVG content.
//!
//! Pipeline:
//!   template — SVG template → minijinja vars → .svg file (pure Rust, no external tool)
//!   raw      — content written verbatim as .svg

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::data_paths;

// ── embedded built-ins ────────────────────────────────────────────────────────

const BUILTIN_SVG_RULES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_svg_rules.rs"));

const BUILTIN_SVG_TEMPLATES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_svg_templates.rs"));

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgBackend {
    Template,
    Raw,
}

impl SvgBackend {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "template" => Ok(Self::Template),
            "raw" | "content" => Ok(Self::Raw),
            other => bail!("unknown backend: {other}. Use: template, raw"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Raw => "raw",
        }
    }
}

impl std::fmt::Display for SvgBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SvgRule {
    pub format: String,
    pub renderer: String,
    pub template: Option<String>,
}

impl SvgRule {
    fn default_for_svg() -> Self {
        Self {
            format: "svg".to_string(),
            renderer: "template".to_string(),
            template: Some("blank".to_string()),
        }
    }
}

/// Metadata from the front matter block in an SVG template.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SvgTemplateMeta {
    pub name: String,
    pub description: String,
    pub variables: Vec<String>,
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
pub struct SvgGenerateRequest {
    pub title: String,
    /// Description stored as .txt alongside the SVG; used as template var or raw content.
    pub description: String,
    pub backend: SvgBackend,
    pub template: Option<String>,
    pub vars: HashMap<String, String>,
    pub output: PathBuf,
    pub workspace: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
    /// Pre-allocated ID (e.g. SVG-000001). Set by the CLI so the ID appears in the filename.
    pub id: Option<String>,
    pub skip_index: bool,
}

pub struct SvgResult {
    pub output: PathBuf,
    pub source: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgEntry {
    pub id: String,
    pub title: String,
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

pub fn generate(req: &SvgGenerateRequest) -> Result<SvgResult> {
    if let Some(parent) = req.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output dir {}", parent.display()))?;
    }

    let source_path = req.output.with_extension("txt");
    if let Err(e) = fs::write(&source_path, &req.description) {
        eprintln!("[orbit svg] warn: could not write source file: {e}");
    }

    let svg_content = match req.backend {
        SvgBackend::Template => render_from_template(req)?,
        SvgBackend::Raw => {
            if req.description.trim().is_empty() {
                bail!("raw backend requires --content with SVG markup");
            }
            req.description.clone()
        }
    };

    fs::write(&req.output, &svg_content)
        .with_context(|| format!("cannot write SVG to {}", req.output.display()))?;

    let bytes = fs::metadata(&req.output).map(|m| m.len()).unwrap_or(0);

    if !req.skip_index {
        let ws = req.workspace.clone().unwrap_or_default();
        let id = req.id.clone().unwrap_or_else(|| next_id(&ws));
        let entry = SvgEntry {
            id,
            title: req.title.clone(),
            backend: req.backend.as_str().to_string(),
            template: req.template.clone(),
            source_path: source_path.clone(),
            output_path: req.output.clone(),
            workspace: ws.clone(),
            tenant: req.tenant.clone().unwrap_or_default(),
            project: req.project.clone().unwrap_or_default(),
            repository: req.repository.clone().unwrap_or_default(),
            vars: req.vars.clone(),
            created_at: now_secs(),
            updated_at: now_secs(),
        };
        if let Err(e) = save_entry(&ws, &entry) {
            eprintln!("[orbit svg] warn: could not save index entry: {e}");
        }
    }

    Ok(SvgResult {
        output: req.output.clone(),
        source: source_path,
        bytes,
    })
}

// ── template backend ──────────────────────────────────────────────────────────

fn render_from_template(req: &SvgGenerateRequest) -> Result<String> {
    let rule = load_svg_rule();
    let tpl_name = req
        .template
        .clone()
        .or_else(|| rule.template.clone())
        .unwrap_or_else(|| "blank".to_string());

    let (_, raw) = resolve_template(&tpl_name)?;
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
    vars.insert("description".to_string(), req.description.clone());
    vars.insert(
        "orbit_workspace".to_string(),
        req.workspace.clone().unwrap_or_default(),
    );
    vars.insert("orbit_scope".to_string(), scope);

    // User vars override auto vars.
    vars.extend(req.vars.clone());

    let meta = parse_template_meta(&raw, &tpl_name);
    for declared in &meta.variables {
        if !vars.contains_key(declared.as_str()) {
            eprintln!(
                "[orbit svg] warn: template variable '{declared}' declared in front matter but not provided (use --var {declared}=VALUE)"
            );
        }
    }

    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    env.add_template("tpl", body)
        .context("invalid SVG template syntax")?;
    let rendered = env
        .get_template("tpl")
        .unwrap()
        .render(vars)
        .context("failed to render SVG template")?;

    Ok(rendered)
}

fn strip_front_matter(src: &str) -> &str {
    let trimmed = src.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<!-- ---")
        && let Some(idx) = rest.find("--- -->")
    {
        let after = &rest[idx + "--- -->".len()..];
        let offset = src.len() - after.len();
        return &src[offset..];
    }
    src
}

// ── rule loading ──────────────────────────────────────────────────────────────

fn load_svg_rule() -> SvgRule {
    if let Some(dir) = data_paths::workspace_svg_rules_dir() {
        let p = dir.join("svg.yaml");
        if let Ok(raw) = fs::read_to_string(&p)
            && let Ok(rule) = serde_yml::from_str::<SvgRule>(&raw)
        {
            return rule;
        }
    }

    let user_p = data_paths::svg_rules_dir().join("svg.yaml");
    if let Ok(raw) = fs::read_to_string(&user_p)
        && let Ok(rule) = serde_yml::from_str::<SvgRule>(&raw)
    {
        return rule;
    }

    for (name, content) in BUILTIN_SVG_RULES {
        if *name == "svg"
            && let Ok(rule) = serde_yml::from_str::<SvgRule>(content)
        {
            return rule;
        }
    }

    SvgRule::default_for_svg()
}

// ── template management ───────────────────────────────────────────────────────

pub fn list_templates() -> Vec<(TemplateSource, SvgTemplateMeta)> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();

    for (_label, dir) in data_paths::scoped_svg_template_dirs() {
        append_templates_from_dir(&dir, &mut seen, &mut out, true);
    }

    let user_dir = data_paths::svg_templates_dir();
    append_templates_from_dir(&user_dir, &mut seen, &mut out, false);

    for (name, content) in BUILTIN_SVG_TEMPLATES {
        if seen.insert(name.to_string()) {
            out.push((TemplateSource::Builtin, parse_template_meta(content, name)));
        }
    }

    out
}

fn append_templates_from_dir(
    dir: &Path,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<(TemplateSource, SvgTemplateMeta)>,
    is_workspace: bool,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "svg"))
        .collect();
    paths.sort_by_key(|e| e.path());
    for entry in paths {
        let name = stem(&entry.path());
        if seen.insert(name.clone())
            && let Ok(raw) = fs::read_to_string(entry.path())
        {
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
    for (label, dir) in data_paths::scoped_svg_template_dirs() {
        let p = dir.join(format!("{name}.svg"));
        if p.exists() {
            let raw = fs::read_to_string(&p)
                .with_context(|| format!("cannot read {label} template {}", p.display()))?;
            return Ok((format!("{label}:{}", p.display()), raw));
        }
    }

    let user_p = data_paths::svg_templates_dir().join(format!("{name}.svg"));
    if user_p.exists() {
        let raw = fs::read_to_string(&user_p)
            .with_context(|| format!("cannot read user template {}", user_p.display()))?;
        return Ok((format!("user:{}", user_p.display()), raw));
    }

    for (tname, content) in BUILTIN_SVG_TEMPLATES {
        if *tname == name {
            return Ok(("builtin".to_string(), content.to_string()));
        }
    }

    bail!("SVG template '{name}' not found. Run `orbit svg template list` to see available.")
}

pub fn add_user_template(path: &Path) -> Result<(String, SvgTemplateMeta)> {
    if path.extension().is_none_or(|e| e != "svg") {
        bail!("template must be an .svg file");
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let name = stem(path);
    let meta = parse_template_meta(&raw, &name);
    let dest_dir = data_paths::svg_templates_dir();
    fs::create_dir_all(&dest_dir).context("cannot create SVG templates directory")?;
    let dest = dest_dir.join(format!("{name}.svg"));
    fs::copy(path, &dest).with_context(|| format!("cannot copy template to {}", dest.display()))?;
    Ok((name, meta))
}

pub fn remove_user_template(name: &str) -> Result<()> {
    let p = data_paths::svg_templates_dir().join(format!("{name}.svg"));
    if !p.exists() {
        bail!("user SVG template not found: {name}");
    }
    fs::remove_file(&p).with_context(|| format!("cannot remove {}", p.display()))
}

fn parse_template_meta(src: &str, fallback: &str) -> SvgTemplateMeta {
    let trimmed = src.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<!-- ---")
        && let Some(end) = rest.find("--- -->")
    {
        let yaml = rest[..end].trim();
        if let Ok(meta) = serde_yml::from_str::<SvgTemplateMeta>(yaml) {
            return meta;
        }
    }
    SvgTemplateMeta {
        name: fallback.to_string(),
        ..Default::default()
    }
}

// ── NDJSON index ──────────────────────────────────────────────────────────────

pub fn now_secs_pub() -> u64 {
    now_secs()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn save_entry(workspace_name: &str, entry: &SvgEntry) -> Result<()> {
    let path = data_paths::svgs_index_path_for(Some(workspace_name));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("cannot create SVG index directory")?;
    }
    let line = serde_json::to_string(entry).context("cannot serialise SVG entry")?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("cannot open index {}", path.display()))?;
    writeln!(f, "{line}").context("cannot write index entry")
}

pub fn load_all_entries(workspace_name: &str) -> Vec<SvgEntry> {
    read_entries(&data_paths::svgs_index_path_for(Some(workspace_name)))
}

pub fn load_all_entries_global() -> Vec<SvgEntry> {
    let root = data_paths::orbit_data_root();
    let mut all: Vec<SvgEntry> = Vec::new();

    let flat = data_paths::svgs_index_path_for(None);
    if flat.exists()
        && let Ok(contents) = fs::read_to_string(&flat)
    {
        for line in contents.lines() {
            if let Ok(e) = serde_json::from_str(line) {
                all.push(e);
            }
        }
    }

    let ws_root = root.join("workspaces");
    if let Ok(entries) = fs::read_dir(&ws_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let idx = entry.path().join("svgs/index.jsonl");
            if let Ok(contents) = fs::read_to_string(&idx) {
                for line in contents.lines() {
                    if let Ok(e) = serde_json::from_str(line) {
                        all.push(e);
                    }
                }
            }
        }
    }
    all
}

pub fn find_entry(query: &str) -> Option<(String, SvgEntry)> {
    let ws = std::env::var("AI_WORKSPACE_ROOT")
        .ok()
        .and_then(|p| {
            PathBuf::from(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_default();

    let mut fallback: Option<(String, SvgEntry)> = None;
    for entry in load_all_entries_global() {
        if entry.id == query
            || entry.output_path.to_string_lossy() == query
            || entry.source_path.to_string_lossy() == query
        {
            if !ws.is_empty() && entry.workspace == ws {
                return Some((entry.workspace.clone(), entry));
            }
            if fallback.is_none() {
                fallback = Some((entry.workspace.clone(), entry));
            }
        }
    }
    fallback
}

pub fn update_stored_entry(workspace_name: &str, id: &str, updated: SvgEntry) -> Result<()> {
    let path = data_paths::svgs_index_path_for(Some(workspace_name));
    let entries: Vec<SvgEntry> = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .map(|e: SvgEntry| if e.id == id { updated.clone() } else { e })
        .collect();
    let mut content = String::new();
    for e in &entries {
        content.push_str(&serde_json::to_string(e)?);
        content.push('\n');
    }
    fs::write(&path, content).with_context(|| format!("cannot rewrite index {}", path.display()))
}

pub fn next_id(workspace_name: &str) -> String {
    format!("SVG-{:06}", load_all_entries(workspace_name).len() + 1)
}

fn read_entries(path: &Path) -> Vec<SvgEntry> {
    let Ok(content) = fs::read_to_string(path) else {
        return vec![];
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rule_loads_builtin() {
        let rule = load_svg_rule();
        assert_eq!(rule.renderer, "template");
    }

    #[test]
    fn template_meta_parsed() {
        let src =
            "<!-- ---\nname: test\ndescription: A test\nvariables:\n  - label\n--- -->\n<svg/>";
        let meta = parse_template_meta(src, "fallback");
        assert_eq!(meta.name, "test");
        assert_eq!(meta.variables, vec!["label"]);
    }

    #[test]
    fn front_matter_stripped() {
        let src = "<!-- ---\nname: test\n--- -->\n<svg/>";
        let body = strip_front_matter(src);
        assert!(!body.contains("<!-- ---"));
        assert!(body.contains("<svg/>"));
    }

    #[test]
    fn generate_raw_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let out = tmp.path().join("SVG-000001-test.svg");
        let req = SvgGenerateRequest {
            title: "Test".into(),
            description: "<svg><text>hello</text></svg>".into(),
            backend: SvgBackend::Raw,
            template: None,
            vars: HashMap::new(),
            output: out.clone(),
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
            id: None,
            skip_index: true,
        };
        let result = generate(&req).unwrap();
        assert!(result.output.exists());
        let content = fs::read_to_string(&result.output).unwrap();
        assert!(content.contains("hello"));
    }

    #[test]
    fn generate_template_backend() {
        let tmp = TempDir::new().unwrap();
        let out = tmp.path().join("SVG-000001-hello.svg");
        let req = SvgGenerateRequest {
            title: "Hello World".into(),
            description: "A test SVG".into(),
            backend: SvgBackend::Template,
            template: None,
            vars: HashMap::new(),
            output: out.clone(),
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
            id: None,
            skip_index: true,
        };
        let result = generate(&req).unwrap();
        let content = fs::read_to_string(&result.output).unwrap();
        assert!(content.contains("Hello World"));
        assert!(result.source.extension().is_some_and(|e| e == "txt"));
    }
}
