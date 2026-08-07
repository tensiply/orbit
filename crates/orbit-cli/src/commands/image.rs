use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use orbit_core::{
    data_paths,
    image::{
        self, ImageBackend, ImageEntry, ImageFormat, ImageGenerateRequest, TemplateSource,
        add_user_template, find_entry, list_templates, load_all_entries, load_all_entries_global,
        next_id, remove_user_template, resolve_template, update_stored_entry,
    },
};
use std::{collections::HashMap, fs, path::PathBuf};

// ── top-level args ─────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct ImageArgs {
    #[command(subcommand)]
    pub subcommand: ImageSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ImageSubcommand {
    /// Generate an image from a template or using AI
    Create(CreateArgs),
    /// List generated images tracked in the index
    List(ListArgs),
    /// Open a generated image by ID or path
    Open(OpenArgs),
    /// Manage image templates
    Template(TemplateArgs),
}

// ── create ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct CreateArgs {
    /// Image title (required)
    #[arg(long, short = 't')]
    pub title: String,

    /// Text content — stored alongside the image and used as template input or AI prompt
    #[arg(long, short = 'c', conflicts_with = "content_file")]
    pub content: Option<String>,

    /// Path to a file containing the text content
    #[arg(long, short = 'f')]
    pub content_file: Option<PathBuf>,

    /// Generation backend: template (HTML+Chrome) or ai (DALL-E 3)
    #[arg(long, short = 'b', default_value = "template")]
    pub backend: String,

    /// Output format: png, jpeg, webp
    #[arg(long, short = 'T', default_value = "png")]
    pub r#type: String,

    /// Template name (for template backend; overrides rule default)
    #[arg(long)]
    pub template: Option<String>,

    /// Image width in pixels (overrides rule default)
    #[arg(long)]
    pub width: Option<u32>,

    /// Image height in pixels (overrides rule default)
    #[arg(long)]
    pub height: Option<u32>,

    /// Template variable: KEY=VALUE (repeatable)
    #[arg(long = "var", short = 'v', value_name = "KEY=VALUE")]
    pub vars: Vec<String>,

    /// Output path. Defaults to `~/.orbit/images/{scope}/{slug}.{ext}`.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Skip backup when the output file already exists (default: backup to .bk)
    #[arg(long)]
    pub force: bool,

    /// Workspace name override (auto-detected from AI_WORKSPACE_ROOT)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct ListArgs {
    /// Filter by workspace name
    #[arg(long)]
    pub workspace: Option<String>,

    /// Maximum number of entries to show
    #[arg(long, default_value = "20")]
    pub limit: usize,
}

// ── open ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct OpenArgs {
    /// Image ID (e.g., IMG-000001) or path to the output file
    pub image: String,
}

// ── template ──────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub subcommand: TemplateSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TemplateSubcommand {
    /// Add a custom template (copies it to the user templates directory)
    Add {
        /// Path to the HTML template file
        path: PathBuf,
    },
    /// List all available templates (builtin + user + workspace)
    List,
    /// Remove a user-defined template (builtins cannot be removed)
    Remove {
        /// Template name (file stem, without .html)
        name: String,
    },
    /// Show the raw HTML source of a template
    Show {
        /// Template name (file stem, without .html)
        name: String,
    },
}

// ── run ───────────────────────────────────────────────────────────────────────

pub fn run(args: ImageArgs) -> Result<()> {
    match args.subcommand {
        ImageSubcommand::Create(a) => run_create(a),
        ImageSubcommand::List(a) => run_list(a),
        ImageSubcommand::Open(a) => run_open(a),
        ImageSubcommand::Template(a) => run_template(a),
    }
}

fn run_create(args: CreateArgs) -> Result<()> {
    let format = ImageFormat::parse(&args.r#type)?;
    let backend = ImageBackend::parse(&args.backend)?;
    let content = resolve_content(args.content.as_deref(), args.content_file.as_deref())?;
    let (workspace_name, tenant, project, repository) = resolve_scope(args.workspace.as_deref());

    // Resolve output path and ID together so the filename can include the ID.
    // Same title → find existing entry → reuse its path and ID (idempotent re-generation).
    // New title → allocate ID first, then build {ID}-{slug}.{ext}.
    let (output, alloc_id, existing): (PathBuf, String, Option<ImageEntry>) =
        if let Some(p) = args.output {
            let ex = find_by_output(&p, &workspace_name);
            let id = ex
                .as_ref()
                .map(|e| e.id.clone())
                .unwrap_or_else(|| next_id(&workspace_name));
            (p, id, ex)
        } else {
            let ex = find_by_title(&args.title, &workspace_name);
            if let Some(ref e) = ex {
                (e.output_path.clone(), e.id.clone(), ex)
            } else {
                let id = next_id(&workspace_name);
                let slug = slugify(&args.title);
                let scope_dir = data_paths::images_scope_dir(
                    &workspace_name,
                    &tenant,
                    &project,
                    &repository,
                );
                let path = scope_dir.join(format!("{id}-{slug}.{}", format.extension()));
                (path, id, None)
            }
        };

    // Backup existing file before overwriting.
    // --force skips the backup and overwrites directly.
    if output.exists() && !args.force {
        let backup = PathBuf::from(format!("{}.bk", output.display()));
        if let Err(e) = fs::copy(&output, &backup) {
            eprintln!("[orbit image] warn: could not backup existing file: {e}");
        }
    }

    let vars = parse_vars(&args.vars)?;

    let req = ImageGenerateRequest {
        title: args.title.clone(),
        text_content: content,
        format,
        backend,
        template: args.template,
        width: args.width,
        height: args.height,
        vars,
        output,
        workspace: Some(workspace_name.clone()),
        tenant: Some(tenant),
        project: Some(project),
        repository: Some(repository),
        // Pass the pre-allocated ID so the core uses it verbatim in the index entry.
        id: Some(alloc_id),
        // If an entry already exists we update it manually below; skip the auto-append.
        skip_index: existing.is_some(),
    };

    let result = image::generate(&req)
        .with_context(|| format!("failed to generate image '{}'", req.title))?;

    // Update existing index entry in-place (preserves the original ID).
    if let Some(entry) = existing {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated = ImageEntry {
            source_path: result.source.clone(),
            updated_at: now,
            ..entry.clone()
        };
        let _ = update_stored_entry(&workspace_name, &entry.id, updated);
    }

    println!(
        "  {} → {} ({} bytes)",
        req.title,
        result.output.display(),
        result.bytes
    );
    println!("  source: {}", result.source.display());

    let dir = result.output.parent().unwrap_or(&result.output);
    if let Err(e) = open::that(dir) {
        eprintln!("[orbit image] warn: could not open directory: {e}");
    }

    Ok(())
}

fn find_by_output(output: &std::path::Path, workspace_name: &str) -> Option<ImageEntry> {
    let target = output.to_string_lossy();
    load_all_entries(workspace_name)
        .into_iter()
        .find(|e| e.output_path.to_string_lossy() == target)
}

fn find_by_title(title: &str, workspace_name: &str) -> Option<ImageEntry> {
    load_all_entries(workspace_name)
        .into_iter()
        .find(|e| e.title == title)
}

fn run_list(args: ListArgs) -> Result<()> {
    let entries: Vec<ImageEntry> = match &args.workspace {
        Some(ws) => load_all_entries(ws),
        None => load_all_entries_global(),
    };

    if entries.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    let shown: Vec<_> = entries.iter().rev().take(args.limit).collect();
    println!(
        "{:<12} {:<12} {:<8} {:<10} {:<24} PATH",
        "ID", "WORKSPACE", "FORMAT", "BACKEND", "TITLE"
    );
    println!("{}", "─".repeat(110));
    for e in shown.iter().rev() {
        let ws_display = if e.workspace.is_empty() {
            "—".to_string()
        } else {
            truncate(&e.workspace, 12)
        };
        println!(
            "{:<12} {:<12} {:<8} {:<10} {:<24} {}",
            e.id,
            ws_display,
            e.format,
            e.backend,
            truncate(&e.title, 24),
            e.output_path.display()
        );
    }

    Ok(())
}

fn run_open(args: OpenArgs) -> Result<()> {
    let (_ws, entry) = find_entry(&args.image)
        .with_context(|| format!("image '{}' not found in the index", args.image))?;

    if !entry.output_path.exists() {
        bail!(
            "output file no longer exists: {}",
            entry.output_path.display()
        );
    }

    open::that(&entry.output_path)
        .with_context(|| format!("cannot open {}", entry.output_path.display()))?;

    Ok(())
}

fn run_template(args: TemplateArgs) -> Result<()> {
    match args.subcommand {
        TemplateSubcommand::Add { path } => {
            let (name, meta) = add_user_template(&path)?;
            println!("  Added template '{name}'");
            if !meta.description.is_empty() {
                println!("  Description: {}", meta.description);
            }
            if !meta.variables.is_empty() {
                println!("  Variables:   {}", meta.variables.join(", "));
            }
            Ok(())
        }
        TemplateSubcommand::List => {
            let templates = list_templates();
            if templates.is_empty() {
                println!("No image templates available.");
                return Ok(());
            }
            println!("{:<22} {:<10} DESCRIPTION", "NAME", "SOURCE");
            println!("{}", "─".repeat(72));
            for (source, meta) in &templates {
                let src_label = match source {
                    TemplateSource::Builtin => "builtin".to_string(),
                    TemplateSource::User(_) => "user".to_string(),
                    TemplateSource::Workspace(_) => "workspace".to_string(),
                };
                println!("{:<22} {:<10} {}", meta.name, src_label, meta.description);
            }
            Ok(())
        }
        TemplateSubcommand::Remove { name } => {
            remove_user_template(&name)?;
            println!("  Removed template '{name}'");
            Ok(())
        }
        TemplateSubcommand::Show { name } => {
            let (source, raw) = resolve_template(&name)?;
            eprintln!("  Template: {name}  source: {source}");
            eprintln!();
            print!("{raw}");
            Ok(())
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn resolve_scope(workspace_arg: Option<&str>) -> (String, String, String, String) {
    let raw_ws = workspace_arg
        .map(|s| s.to_string())
        .or_else(|| std::env::var("AI_WORKSPACE_ROOT").ok())
        .unwrap_or_default();
    let workspace_name = PathBuf::from(&raw_ws)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&raw_ws)
        .to_string();
    let tenant = std::env::var("AI_TENANT").unwrap_or_default();
    let project = std::env::var("AI_PROJECT").unwrap_or_default();
    let repository = std::env::var("AI_REPOSITORY").unwrap_or_default();
    (workspace_name, tenant, project, repository)
}

fn resolve_content(
    content: Option<&str>,
    content_file: Option<&std::path::Path>,
) -> Result<String> {
    match (content, content_file) {
        (Some(c), _) => Ok(c.to_string()),
        (None, Some(path)) => fs::read_to_string(path)
            .with_context(|| format!("cannot read content file {}", path.display())),
        (None, None) => Ok(String::new()),
    }
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn parse_vars(raw: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for entry in raw {
        let (k, v) = entry
            .split_once('=')
            .with_context(|| format!("invalid --var format: '{entry}'. Expected KEY=VALUE"))?;
        map.insert(k.trim().to_string(), v.to_string());
    }
    Ok(map)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
