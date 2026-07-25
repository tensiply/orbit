//! Document generation: YAML rules, HTML templates, and format-specific renderers.
//!
//! Pipeline:
//!   PDF   — markdown → pulldown-cmark → HTML → minijinja(template) → weasyprint → PDF
//!   HTML  — markdown → pulldown-cmark → HTML → minijinja(template) → file
//!   DOCX  — markdown content written to temp file → pandoc
//!   XLSX  — JSON/CSV content → rust_xlsxwriter
//!   CSV   — raw content → file (pure Rust)

use anyhow::{Context, Result, bail};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{data_paths, user_config::UserConfig};

// ── embedded built-ins ────────────────────────────────────────────────────────

const BUILTIN_DOCUMENT_RULES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_document_rules.rs"));

const BUILTIN_DOCUMENT_TEMPLATES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_document_templates.rs"));

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentFormat {
    Pdf,
    Html,
    Docx,
    Xlsx,
    Csv,
}

impl DocumentFormat {
    /// Parse from string, accepting aliases (excel→Xlsx, word→Docx).
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pdf" => Ok(Self::Pdf),
            "html" => Ok(Self::Html),
            "docx" | "word" => Ok(Self::Docx),
            "xlsx" | "xls" | "excel" => Ok(Self::Xlsx),
            "csv" => Ok(Self::Csv),
            other => bail!("unknown document type: {other}. Use: pdf, html, docx, xlsx, csv"),
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct DocumentRule {
    pub format: String,
    /// Renderer backend: weasyprint | pandoc | xlsxwriter | builtin
    pub renderer: String,
    /// Default template name for HTML/PDF rendering.
    pub template: Option<String>,
    /// Page size for PDF (e.g., "A4", "Letter").
    pub page_size: Option<String>,
    pub margins: Option<Margins>,
    pub pdf_options: Option<PdfOptions>,
    /// Optional pandoc reference document for DOCX styling (reserved for future use).
    pub reference_doc: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PdfOptions {
    #[serde(default = "default_dpi")]
    pub dpi: u32,
    #[serde(default = "default_zoom")]
    pub zoom: f32,
}

fn default_dpi() -> u32 {
    150
}
fn default_zoom() -> f32 {
    1.0
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            dpi: 150,
            zoom: 1.0,
        }
    }
}

/// Metadata parsed from the YAML front matter block in an HTML template.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct DocumentTemplateMeta {
    pub name: String,
    pub description: String,
    /// Declared user-defined variables. Missing declared vars emit a warning at render time.
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateSource {
    Builtin,
    /// Template from `~/.local/share/orbit/templates/documents/`
    User(PathBuf),
    /// Template from `$AI_CONTEXT_ROOT/templates/document/` (workspace-scoped)
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

/// All inputs needed to generate a document.
pub struct GenerateRequest {
    pub title: String,
    pub format: DocumentFormat,
    /// Raw markdown / JSON / CSV content.
    pub content: String,
    /// Template name override (falls back to rule's default template).
    pub template: Option<String>,
    /// Output file path.
    pub output: PathBuf,
    /// User-provided template variables (from --var KEY=VAL).
    pub vars: HashMap<String, String>,
    /// Workspace name (for orbit_workspace auto-var and index).
    pub workspace: Option<String>,
    /// Tenant name derived from AI_TENANT.
    pub tenant: Option<String>,
    /// Project name derived from AI_PROJECT.
    pub project: Option<String>,
    /// Repository name derived from AI_REPOSITORY.
    pub repository: Option<String>,
    /// Explicit scope string override (for orbit_scope auto-var).
    pub scope: Option<String>,
    /// When true, skip writing to the NDJSON document index (used by update flow).
    pub skip_index: bool,
}

pub struct GenerateResult {
    pub output: PathBuf,
    /// Source file written alongside the output (e.g., .md for PDF/HTML/DOCX).
    pub source: PathBuf,
    pub bytes: u64,
}

// ── document index (NDJSON) ───────────────────────────────────────────────────

/// A record stored in the NDJSON document index.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentEntry {
    pub id: String,
    pub title: String,
    pub format: String,
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

/// Public accessor — used by CLI update flow to stamp `updated_at`.
pub fn now_secs_pub() -> u64 {
    now_secs()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Append one entry to the NDJSON index for the given workspace.
pub fn save_entry(workspace_name: &str, entry: &DocumentEntry) -> Result<()> {
    use std::io::Write;
    let path = data_paths::documents_index_path_for(Some(workspace_name));
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Load all entries from the NDJSON index for a workspace.
pub fn load_all_entries(workspace_name: &str) -> Vec<DocumentEntry> {
    let path = data_paths::documents_index_path_for(Some(workspace_name));
    if !path.exists() {
        return Vec::new();
    }
    fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Load all entries across all workspaces (searches all workspace slugs).
pub fn load_all_entries_global() -> Vec<DocumentEntry> {
    let root = data_paths::orbit_data_root();
    let mut all: Vec<DocumentEntry> = Vec::new();

    // Legacy flat index
    let flat = data_paths::documents_index_path_for(None);
    if flat.exists()
        && let Ok(contents) = fs::read_to_string(&flat)
    {
        for line in contents.lines() {
            if let Ok(e) = serde_json::from_str(line) {
                all.push(e);
            }
        }
    }

    // Per-workspace indexes
    let ws_root = root.join("workspaces");
    if let Ok(entries) = fs::read_dir(&ws_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let idx = entry.path().join("documents/index.jsonl");
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

/// Find an entry by ID (e.g., "DOC-000001") or by output/source path.
pub fn find_entry(id_or_path: &str) -> Option<(String, DocumentEntry)> {
    for entry in load_all_entries_global() {
        if entry.id == id_or_path
            || entry.output_path.to_string_lossy() == id_or_path
            || entry.source_path.to_string_lossy() == id_or_path
        {
            let ws = entry.workspace.clone();
            return Some((ws, entry));
        }
    }
    None
}

/// Generate the next document ID for a workspace in DOC-NNNNNN format.
pub fn next_id(workspace_name: &str) -> String {
    let count = load_all_entries(workspace_name).len();
    format!("DOC-{:06}", count + 1)
}

/// Overwrite an existing entry in the NDJSON index (rewrites the whole file).
pub fn update_stored_entry(workspace_name: &str, id: &str, updated: DocumentEntry) -> Result<()> {
    let path = data_paths::documents_index_path_for(Some(workspace_name));
    let entries: Vec<DocumentEntry> = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .map(|e: DocumentEntry| if e.id == id { updated.clone() } else { e })
        .collect();
    let mut content = String::new();
    for e in &entries {
        content.push_str(&serde_json::to_string(e)?);
        content.push('\n');
    }
    fs::write(&path, content)?;
    Ok(())
}

// ── rule loading ──────────────────────────────────────────────────────────────

/// Load the document rule for a format.
///
/// Priority: user override (`~/.config/orbit/document-rules/{format}.yaml`) → builtin.
pub fn load_rule(format: &DocumentFormat) -> DocumentRule {
    let format_str = format.as_str();
    let user_path = data_paths::document_rules_dir().join(format!("{format_str}.yaml"));

    if user_path.exists()
        && let Ok(content) = fs::read_to_string(&user_path)
        && let Ok(rule) = serde_yml::from_str::<DocumentRule>(&content)
    {
        return rule;
    }

    for (name, content) in BUILTIN_DOCUMENT_RULES {
        if *name == format_str
            && let Ok(rule) = serde_yml::from_str::<DocumentRule>(content)
        {
            return rule;
        }
    }

    // Hardcoded fallback
    DocumentRule {
        format: format_str.to_string(),
        renderer: match format {
            DocumentFormat::Pdf => "weasyprint",
            DocumentFormat::Html => "builtin",
            DocumentFormat::Docx => "pandoc",
            DocumentFormat::Xlsx => "xlsxwriter",
            DocumentFormat::Csv => "builtin",
        }
        .to_string(),
        ..Default::default()
    }
}

// ── template loading ──────────────────────────────────────────────────────────

/// Strip the `<!-- --- ... --- -->` front matter block and parse it.
///
/// Returns `(meta, html_body)`. If no front matter is found, meta is default.
pub fn parse_template_front_matter(source: &str) -> (DocumentTemplateMeta, String) {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("<!-- ---") {
        return (DocumentTemplateMeta::default(), source.to_string());
    }

    if let Some(end) = trimmed.find("--- -->") {
        let inner_start = "<!-- ---".len();
        let inner = trimmed[inner_start..end].trim_matches(|c: char| c == '\n' || c == '\r');
        let meta: DocumentTemplateMeta = serde_yml::from_str(inner).unwrap_or_default();
        let body = trimmed[end + "--- -->".len()..]
            .trim_start_matches('\n')
            .to_string();
        (meta, body)
    } else {
        (DocumentTemplateMeta::default(), source.to_string())
    }
}

/// List all available templates (builtin + user).
///
/// User templates with the same name as a builtin override it in the list.
/// List all available templates.
///
/// Precedence (highest first): workspace → user → builtin.
/// A higher-priority template with the same name shadows lower ones.
pub fn list_templates() -> Vec<(TemplateSource, DocumentTemplateMeta)> {
    let mut results: Vec<(TemplateSource, DocumentTemplateMeta)> = Vec::new();

    for (_, content) in BUILTIN_DOCUMENT_TEMPLATES {
        let (meta, _) = parse_template_front_matter(content);
        results.push((TemplateSource::Builtin, meta));
    }

    let user_dir = data_paths::document_templates_dir();
    collect_html_templates(&user_dir, |path, meta| {
        results.retain(|(_, m)| m.name != meta.name);
        results.push((TemplateSource::User(path), meta));
    });

    if let Some(ws_dir) = data_paths::workspace_document_templates_dir() {
        collect_html_templates(&ws_dir, |path, meta| {
            results.retain(|(_, m)| m.name != meta.name);
            results.push((TemplateSource::Workspace(path), meta));
        });
    }

    results
}

fn collect_html_templates(dir: &std::path::Path, mut push: impl FnMut(PathBuf, DocumentTemplateMeta)) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
            .collect();
        paths.sort_by_key(|e| e.path());
        for entry in paths {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                let (meta, _) = parse_template_front_matter(&content);
                push(entry.path(), meta);
            }
        }
    }
}

/// Resolve a template by name, returning `(source, raw_html_source)`.
///
/// Precedence: workspace dir → user dir → builtin.
pub fn resolve_template(name: &str) -> Result<(TemplateSource, String)> {
    // 1. workspace-scoped templates ($AI_CONTEXT_ROOT/templates/document/)
    if let Some(ws_dir) = data_paths::workspace_document_templates_dir() {
        let ws_path = ws_dir.join(format!("{name}.html"));
        if ws_path.exists() {
            let content = fs::read_to_string(&ws_path)
                .with_context(|| format!("failed to read template {}", ws_path.display()))?;
            return Ok((TemplateSource::Workspace(ws_path), content));
        }
    }

    // 2. user local templates (~/.local/share/orbit/templates/documents/)
    let user_dir = data_paths::document_templates_dir();
    let user_path = user_dir.join(format!("{name}.html"));
    if user_path.exists() {
        let content = fs::read_to_string(&user_path)
            .with_context(|| format!("failed to read template {}", user_path.display()))?;
        return Ok((TemplateSource::User(user_path), content));
    }

    // 3. built-in templates
    for (tpl_name, content) in BUILTIN_DOCUMENT_TEMPLATES {
        if *tpl_name == name {
            return Ok((TemplateSource::Builtin, content.to_string()));
        }
    }

    bail!(
        "template '{name}' not found. Use `orbit document template list` to see available templates."
    )
}

// ── variable pipeline ─────────────────────────────────────────────────────────

fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

fn auto_vars(req: &GenerateRequest) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    vars.insert("title".into(), req.title.clone());
    vars.insert("date".into(), iso_date_now());
    vars.insert("timestamp_human".into(), human_timestamp_now());

    let user = UserConfig::load().user;
    let author = user.name.clone();
    if !author.is_empty() {
        vars.insert("author".into(), author);
    }
    let display_name = if !user.display_name.is_empty() {
        user.display_name.clone()
    } else {
        user.name.clone()
    };
    vars.insert("display_name".into(), display_name);

    vars.insert("content_raw".into(), req.content.clone());
    vars.insert("content_html".into(), markdown_to_html(&req.content));

    // Workspace: prefer provided name; if it looks like a path, take the last component.
    let raw_ws = req
        .workspace
        .clone()
        .or_else(|| std::env::var("AI_WORKSPACE_ROOT").ok())
        .unwrap_or_default();
    let workspace = PathBuf::from(&raw_ws)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&raw_ws)
        .to_string();
    vars.insert("orbit_workspace".into(), workspace.clone());
    vars.insert("workspace_pascal".into(), to_pascal_case(&workspace));

    let tenant = req
        .tenant
        .clone()
        .or_else(|| std::env::var("AI_TENANT").ok())
        .unwrap_or_default();
    vars.insert("tenant_pascal".into(), to_pascal_case(&tenant));

    let scope = req.scope.clone().unwrap_or_else(|| {
        let p = req
            .project
            .clone()
            .or_else(|| std::env::var("AI_PROJECT").ok())
            .unwrap_or_default();
        let r = req
            .repository
            .clone()
            .or_else(|| std::env::var("AI_REPOSITORY").ok())
            .unwrap_or_default();
        [tenant.clone(), p, r]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("/")
    });
    vars.insert("orbit_scope".into(), scope);

    vars
}

fn iso_date_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days as u32);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Human-readable timestamp: "Jul 25, 2026 at 14:32"
fn human_timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hour = time_secs / 3600;
    let minute = (time_secs % 3600) / 60;
    let (y, m, d) = days_to_ymd(days as u32);
    let month_abbr = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                      "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mon = month_abbr.get(m.saturating_sub(1) as usize).unwrap_or(&"???");
    format!("{mon} {d}, {y} at {hour:02}:{minute:02}")
}

fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let months = [0u32, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    loop {
        let dm = if month == 2 && is_leap(year) {
            29
        } else {
            months[month as usize]
        };
        if days < dm {
            break;
        }
        days -= dm;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(markdown, opts);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

// ── main generate entry point ─────────────────────────────────────────────────

/// Extension of the source file stored alongside the generated document.
fn source_ext_for(format: &DocumentFormat) -> &'static str {
    match format {
        DocumentFormat::Xlsx => "json",
        DocumentFormat::Csv => "csv",
        _ => "md",
    }
}

/// Generate a document from the request and write it to `req.output`.
///
/// Also writes a source file (same directory, same stem, different extension)
/// and appends an entry to the workspace NDJSON document index.
pub fn generate(req: &GenerateRequest) -> Result<GenerateResult> {
    if let Some(parent) = req.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output dir {}", parent.display()))?;
    }

    // Determine source file path alongside the output.
    let src_ext = source_ext_for(&req.format);
    let source = req
        .output
        .file_stem()
        .map(|stem| {
            req.output
                .with_file_name(format!("{}.{src_ext}", stem.to_string_lossy()))
        })
        .unwrap_or_else(|| req.output.with_extension(src_ext));

    // Write source file (non-fatal on error).
    if source != req.output
        && let Err(e) = fs::write(&source, &req.content)
    {
        eprintln!("[orbit document] warn: could not write source file: {e}");
    }

    let rule = load_rule(&req.format);

    let bytes = match req.format {
        DocumentFormat::Pdf => generate_pdf(req, &rule)?,
        DocumentFormat::Html => generate_html(req, &rule)?,
        DocumentFormat::Docx => generate_docx(req, &rule)?,
        DocumentFormat::Xlsx => generate_xlsx(req)?,
        DocumentFormat::Csv => generate_csv(req)?,
    };

    // Extract workspace name (last path component if it looks like a path).
    let raw_ws = req
        .workspace
        .clone()
        .or_else(|| std::env::var("AI_WORKSPACE_ROOT").ok())
        .unwrap_or_default();
    let workspace_name = PathBuf::from(&raw_ws)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&raw_ws)
        .to_string();

    let entry = DocumentEntry {
        id: next_id(&workspace_name),
        title: req.title.clone(),
        format: req.format.as_str().to_string(),
        template: req.template.clone(),
        source_path: source.clone(),
        output_path: req.output.clone(),
        workspace: workspace_name.clone(),
        tenant: req
            .tenant
            .clone()
            .or_else(|| std::env::var("AI_TENANT").ok())
            .unwrap_or_default(),
        project: req
            .project
            .clone()
            .or_else(|| std::env::var("AI_PROJECT").ok())
            .unwrap_or_default(),
        repository: req
            .repository
            .clone()
            .or_else(|| std::env::var("AI_REPOSITORY").ok())
            .unwrap_or_default(),
        vars: req.vars.clone(),
        created_at: now_secs(),
        updated_at: now_secs(),
    };

    if !req.skip_index
        && let Err(e) = save_entry(&workspace_name, &entry)
    {
        eprintln!("[orbit document] warn: could not save document index entry: {e}");
    }

    Ok(GenerateResult {
        output: req.output.clone(),
        source,
        bytes,
    })
}

// ── renderers ─────────────────────────────────────────────────────────────────

fn render_template(req: &GenerateRequest, rule: &DocumentRule) -> Result<String> {
    let template_name = req
        .template
        .clone()
        .or_else(|| rule.template.clone())
        .unwrap_or_else(|| "default".into());

    let (_, raw) = resolve_template(&template_name)?;
    let (meta, html_body) = parse_template_front_matter(&raw);

    let mut ctx: HashMap<String, String> = auto_vars(req);
    ctx.extend(req.vars.clone());

    for declared in &meta.variables {
        if !ctx.contains_key(declared.as_str()) {
            eprintln!(
                "[orbit document] warn: template variable '{declared}' declared in front matter but not provided (use --var {declared}=VALUE)"
            );
        }
    }

    use minijinja::Environment;
    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    env.add_template("doc", &html_body)
        .context("failed to parse template")?;
    let tmpl = env.get_template("doc").unwrap();
    let rendered = tmpl.render(&ctx).context("failed to render template")?;

    Ok(rendered)
}

fn generate_html(req: &GenerateRequest, rule: &DocumentRule) -> Result<u64> {
    let html = render_template(req, rule)?;
    fs::write(&req.output, &html)
        .with_context(|| format!("failed to write {}", req.output.display()))?;
    Ok(html.len() as u64)
}

struct PdfRenderCtx {
    /// Footer left: "WorkspacePascal TenantPascal — Title — Date"
    footer_left: String,
    /// Footer left line 2: "Generated by Name with orbit documents"
    footer_left2: String,
    /// Page size hint (e.g. "A4") — used by wkhtmltopdf; weasyprint uses @page CSS.
    page_size: String,
}

fn generate_pdf(req: &GenerateRequest, rule: &DocumentRule) -> Result<u64> {
    let html = render_template(req, rule)?;

    let tmp_dir = std::env::temp_dir().join("orbit-document");
    fs::create_dir_all(&tmp_dir)?;
    let tmp_html = tmp_dir.join(format!("orbit-doc-{}.html", random_suffix()));
    fs::write(&tmp_html, &html)
        .with_context(|| format!("failed to write temp HTML {}", tmp_html.display()))?;

    // Build footer context from auto_vars so values match what the template sees.
    let av = auto_vars(req);
    let timestamp_human = av.get("timestamp_human").cloned().unwrap_or_default();
    let display_name = av.get("display_name").cloned().unwrap_or_default();

    let footer_left = if display_name.is_empty() {
        format!("Generated at {timestamp_human} with orbit documents")
    } else {
        format!("Generated at {timestamp_human} by {display_name} with orbit documents")
    };
    let footer_left2 = {
        // unused for wkhtmltopdf (single-line footer), kept for struct compat
        String::new()
    };

    let ctx = PdfRenderCtx {
        footer_left,
        footer_left2,
        page_size: rule.page_size.clone().unwrap_or_else(|| "A4".into()),
    };

    let render_result = try_pdf_renderers(&tmp_html, &req.output, &ctx);
    let _ = fs::remove_file(&tmp_html);
    render_result?;

    let metadata = fs::metadata(&req.output)
        .with_context(|| format!("PDF not found at {}", req.output.display()))?;
    Ok(metadata.len())
}

/// Try PDF renderers in order: weasyprint (venv) → wkhtmltopdf → weasyprint (system).
///
/// weasyprint handles margins and footer via CSS `@page` in the template.
/// wkhtmltopdf receives explicit margin/footer flags from `ctx`.
/// To fix weasyprint on Homebrew Python: `brew install pango`.
fn try_pdf_renderers(html: &Path, output: &Path, ctx: &PdfRenderCtx) -> Result<()> {
    // 1. weasyprint from orbit venv — probe silently; fall through on any failure.
    let weasyprint_venv = crate::venv::venv_bin("weasyprint");
    if weasyprint_venv.exists() {
        if Command::new(&weasyprint_venv)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            let status = Command::new(&weasyprint_venv)
                .arg(html)
                .arg(output)
                .status()
                .context("failed to run weasyprint")?;
            if status.success() {
                return Ok(());
            }
            bail!("weasyprint failed to generate PDF");
        }
        // probe failed (missing system libs) — fall through silently to next renderer
    }

    // 2. wkhtmltopdf — margins and footer via CLI flags (template @page CSS is ignored).
    if let Ok(wp) = which_binary("wkhtmltopdf") {
        let footer_line = format!("{}  |  {}", ctx.footer_left, ctx.footer_left2);
        let status = Command::new(&wp)
            .arg("--quiet")
            .arg("--page-size").arg(&ctx.page_size)
            .arg("--margin-top").arg("25")
            .arg("--margin-bottom").arg("28")
            .arg("--margin-left").arg("20")
            .arg("--margin-right").arg("20")
            .arg("--footer-font-size").arg("8")
            .arg("--footer-spacing").arg("3")
            .arg("--footer-left").arg(&footer_line)
            .arg("--footer-right").arg("Page [page] / [topage]")
            .arg(html)
            .arg(output)
            .status()
            .context("failed to run wkhtmltopdf")?;
        if status.success() {
            return Ok(());
        }
        bail!("wkhtmltopdf failed to generate PDF");
    }

    // 3. weasyprint from system PATH (installed via apt/brew at system level)
    if let Ok(wp) = which_binary("weasyprint") {
        let status = Command::new(&wp)
            .arg(html)
            .arg(output)
            .status()
            .context("failed to run weasyprint (system)")?;
        if status.success() {
            return Ok(());
        }
        bail!("weasyprint (system) failed to generate PDF");
    }

    bail!(
        "no PDF renderer found.\n\
         Install one of:\n  \
         sudo apt install wkhtmltopdf\n  \
         brew install pango  (fixes weasyprint on Homebrew Python)\n  \
         sudo apt install weasyprint"
    )
}

fn which_binary(name: &str) -> Result<PathBuf> {
    let out = Command::new("which").arg(name).output()?;
    if out.status.success() {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(PathBuf::from(path))
    } else {
        bail!("{name} not found in PATH")
    }
}

fn generate_docx(req: &GenerateRequest, rule: &DocumentRule) -> Result<u64> {
    let tmp_dir = std::env::temp_dir().join("orbit-document");
    fs::create_dir_all(&tmp_dir)?;
    let tmp_md = tmp_dir.join(format!("orbit-doc-{}.md", random_suffix()));
    fs::write(&tmp_md, &req.content)
        .with_context(|| format!("failed to write temp markdown {}", tmp_md.display()))?;

    let mut cmd = Command::new("pandoc");
    cmd.arg(&tmp_md)
        .arg("-o")
        .arg(&req.output)
        .arg("--from")
        .arg("markdown")
        .arg("--to")
        .arg("docx");

    if let Some(ref_doc) = rule.reference_doc.as_ref()
        && ref_doc.exists()
    {
        cmd.arg(format!("--reference-doc={}", ref_doc.display()));
    }

    let status = cmd.status().with_context(
        || "failed to run pandoc. Install it with: brew install pandoc  or  apt install pandoc",
    )?;

    if !status.success() {
        bail!("pandoc failed to generate DOCX");
    }

    let _ = fs::remove_file(&tmp_md);

    let metadata = fs::metadata(&req.output)
        .with_context(|| format!("DOCX not found at {}", req.output.display()))?;
    Ok(metadata.len())
}

fn generate_xlsx(req: &GenerateRequest) -> Result<u64> {
    use rust_xlsxwriter::Workbook;

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    let sheet_name: String = req.title.chars().take(31).collect();
    ws.set_name(&sheet_name)?;

    if let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&req.content)
        && let Some(first) = rows.first()
        && let Some(obj) = first.as_object()
    {
        let headers: Vec<String> = obj.keys().cloned().collect();
        for (col, header) in headers.iter().enumerate() {
            ws.write_string(0, col as u16, header)?;
        }
        for (row_idx, row) in rows.iter().enumerate() {
            if let Some(obj) = row.as_object() {
                for (col, header) in headers.iter().enumerate() {
                    let val = obj.get(header).cloned().unwrap_or(serde_json::Value::Null);
                    match val {
                        serde_json::Value::Number(n) => {
                            if let Some(f) = n.as_f64() {
                                ws.write_number(row_idx as u32 + 1, col as u16, f)?;
                            }
                        }
                        serde_json::Value::Bool(b) => {
                            ws.write_boolean(row_idx as u32 + 1, col as u16, b)?;
                        }
                        serde_json::Value::Null => {}
                        other => {
                            ws.write_string(
                                row_idx as u32 + 1,
                                col as u16,
                                other.to_string().trim_matches('"'),
                            )?;
                        }
                    }
                }
            }
        }
    } else {
        // Fallback: treat content as CSV
        for (row_idx, line) in req.content.lines().enumerate() {
            for (col_idx, cell) in line.split(',').enumerate() {
                ws.write_string(row_idx as u32, col_idx as u16, cell.trim())?;
            }
        }
    }

    wb.save(&req.output)
        .with_context(|| format!("failed to write XLSX to {}", req.output.display()))?;

    let metadata = fs::metadata(&req.output)?;
    Ok(metadata.len())
}

fn generate_csv(req: &GenerateRequest) -> Result<u64> {
    let content = req.content.as_bytes();
    let mut file = fs::File::create(&req.output)
        .with_context(|| format!("failed to create {}", req.output.display()))?;
    file.write_all(content)?;
    Ok(content.len() as u64)
}

fn random_suffix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
}

// ── template management ───────────────────────────────────────────────────────

/// Copy a template file into the user templates directory.
///
/// Returns the template name (file stem) and its declared metadata.
pub fn add_user_template(path: &Path) -> Result<(String, DocumentTemplateMeta)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if ext != "html" {
        bail!("template must be an HTML file (.html), got: {ext}");
    }

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("invalid filename")?
        .to_string();

    let content =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let (meta, _) = parse_template_front_matter(&content);

    let dest_dir = data_paths::document_templates_dir();
    fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(format!("{name}.html"));
    fs::copy(path, &dest).with_context(|| format!("failed to copy to {}", dest.display()))?;

    Ok((name, meta))
}

/// Remove a user template. Refuses to remove builtins (they live in the binary).
pub fn remove_user_template(name: &str) -> Result<()> {
    let user_dir = data_paths::document_templates_dir();
    let path = user_dir.join(format!("{name}.html"));

    if !path.exists() {
        for (tpl_name, _) in BUILTIN_DOCUMENT_TEMPLATES {
            if *tpl_name == name {
                bail!("'{name}' is a built-in template and cannot be removed");
            }
        }
        bail!("template '{name}' not found in user templates directory");
    }

    fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rule_loads_builtin_pdf() {
        let rule = load_rule(&DocumentFormat::Pdf);
        assert_eq!(rule.renderer, "weasyprint");
    }

    #[test]
    fn rule_loads_builtin_csv() {
        let rule = load_rule(&DocumentFormat::Csv);
        assert_eq!(rule.renderer, "builtin");
    }

    #[test]
    fn template_front_matter_stripped() {
        let src = "<!-- ---\nname: test\ndescription: A test template\nvariables:\n  - custom_var\n--- -->\n<html><body>{{ title }}</body></html>";
        let (meta, body) = parse_template_front_matter(src);
        assert_eq!(meta.name, "test");
        assert_eq!(meta.variables, vec!["custom_var"]);
        assert!(body.contains("<html>"));
        assert!(!body.contains("<!-- ---"));
    }

    #[test]
    fn template_no_front_matter_passthrough() {
        let src = "<html><body>{{ title }}</body></html>";
        let (meta, body) = parse_template_front_matter(src);
        assert_eq!(meta.name, "");
        assert_eq!(body, src);
    }

    #[test]
    fn auto_vars_contain_required_keys() {
        let req = GenerateRequest {
            title: "Test Doc".into(),
            format: DocumentFormat::Html,
            content: "# Hello".into(),
            template: None,
            output: PathBuf::from("/tmp/test.html"),
            vars: HashMap::new(),
            workspace: Some("TestWS".into()),
            tenant: None,
            project: None,
            repository: None,
            scope: Some("T/P/R".into()),
            skip_index: false,
        };
        let vars = auto_vars(&req);
        assert!(vars.contains_key("title"));
        assert!(vars.contains_key("date"));
        assert!(vars.contains_key("content_html"));
        assert!(vars.contains_key("content_raw"));
        assert_eq!(vars["orbit_workspace"], "TestWS");
        assert_eq!(vars["orbit_scope"], "T/P/R");
    }

    #[test]
    fn generate_csv_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let out = tmp.path().join("out.csv");
        let req = GenerateRequest {
            title: "Test".into(),
            format: DocumentFormat::Csv,
            content: "a,b,c\n1,2,3\n".into(),
            template: None,
            output: out.clone(),
            vars: HashMap::new(),
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
            scope: None,
            skip_index: true,
        };
        let result = generate(&req).unwrap();
        assert!(result.output.exists());
        let content = fs::read_to_string(&result.output).unwrap();
        assert_eq!(content, "a,b,c\n1,2,3\n");
    }

    #[test]
    fn generate_html_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let out = tmp.path().join("out.html");
        let req = GenerateRequest {
            title: "Hello World".into(),
            format: DocumentFormat::Html,
            content: "# Hello\nThis is content.".into(),
            template: None,
            output: out.clone(),
            vars: HashMap::new(),
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
            scope: None,
            skip_index: true,
        };
        let result = generate(&req).unwrap();
        let html = fs::read_to_string(&result.output).unwrap();
        assert!(html.contains("Hello World"));
        assert!(html.contains("<h1>"));
    }

    #[test]
    fn generate_result_has_source() {
        let tmp = TempDir::new().unwrap();
        let out = tmp.path().join("out.html");
        let req = GenerateRequest {
            title: "Source Test".into(),
            format: DocumentFormat::Html,
            content: "# Source".into(),
            template: None,
            output: out.clone(),
            vars: HashMap::new(),
            workspace: None,
            tenant: None,
            project: None,
            repository: None,
            scope: None,
            skip_index: true,
        };
        let result = generate(&req).unwrap();
        assert!(result.source.exists(), "source file should be written");
        assert_eq!(result.source.extension().unwrap(), "md");
    }

    #[test]
    fn document_format_aliases() {
        assert_eq!(
            DocumentFormat::parse("excel").unwrap(),
            DocumentFormat::Xlsx
        );
        assert_eq!(DocumentFormat::parse("word").unwrap(), DocumentFormat::Docx);
        assert_eq!(DocumentFormat::parse("PDF").unwrap(), DocumentFormat::Pdf);
    }

    #[test]
    fn user_template_parse_override() {
        let tmp = TempDir::new().unwrap();
        let tpl_path = tmp.path().join("default.html");
        fs::write(
            &tpl_path,
            "<!-- ---\nname: default\ndescription: Custom override\n--- -->\n<html><body>CUSTOM {{ title }}</body></html>",
        )
        .unwrap();

        let content = fs::read_to_string(&tpl_path).unwrap();
        let (meta, body) = parse_template_front_matter(&content);
        assert_eq!(meta.name, "default");
        assert_eq!(meta.description, "Custom override");
        assert!(body.contains("CUSTOM"));
    }
}
