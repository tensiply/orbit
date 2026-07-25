use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use orbit_core::{
    data_paths,
    document::{
        self, DocumentEntry, DocumentFormat, GenerateRequest, TemplateSource, add_user_template,
        find_entry, list_templates, load_all_entries, load_all_entries_global,
        remove_user_template, resolve_template, update_stored_entry,
    },
};
use std::{collections::HashMap, fs, path::PathBuf};

// ── top-level args ─────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct DocumentArgs {
    #[command(subcommand)]
    pub subcommand: DocumentSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum DocumentSubcommand {
    /// Generate a document from markdown or data content
    Create(CreateArgs),
    /// Edit an existing document: backup source, apply new content, regenerate
    Update(UpdateArgs),
    /// List generated documents tracked in the index
    List(ListArgs),
    /// Manage document templates
    Template(TemplateArgs),
}

// ── create ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct CreateArgs {
    /// Document title (required)
    #[arg(long, short = 't')]
    pub title: String,

    /// Output format: pdf, html, docx, xlsx, csv  [default: pdf]
    #[arg(long, short = 'T', default_value = "pdf")]
    pub r#type: String,

    /// Document content (markdown, JSON for xlsx, raw for csv)
    #[arg(long, short = 'c', conflicts_with = "content_file")]
    pub content: Option<String>,

    /// Path to a file containing the document content
    #[arg(long, short = 'f')]
    pub content_file: Option<PathBuf>,

    /// Template name to use (overrides rule default)
    #[arg(long)]
    pub template: Option<String>,

    /// Output path. Defaults to `~/.orbit/documents/{scope}/{slug}.{ext}`.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Template variable: KEY=VALUE (repeatable)
    #[arg(long = "var", short = 'v', value_name = "KEY=VALUE")]
    pub vars: Vec<String>,

    /// Open the output directory in the file explorer after creation
    #[arg(long)]
    pub open: bool,

    /// Overwrite the output file if it already exists
    #[arg(long)]
    pub force: bool,

    /// Workspace name override (auto-detected from AI_WORKSPACE_ROOT)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ── update ────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct UpdateArgs {
    /// Document ID (e.g., DOC-000001) or path to the output file
    pub document: String,

    /// New document content (markdown / JSON / CSV)
    #[arg(long, short = 'c', conflicts_with = "content_file")]
    pub content: Option<String>,

    /// Path to a file containing the new document content
    #[arg(long, short = 'f')]
    pub content_file: Option<PathBuf>,

    /// Template variable: KEY=VALUE (repeatable)
    #[arg(long = "var", short = 'v', value_name = "KEY=VALUE")]
    pub vars: Vec<String>,

    /// Template name override
    #[arg(long)]
    pub template: Option<String>,

    /// Open the output directory in the file explorer after updating
    #[arg(long)]
    pub open: bool,
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct ListArgs {
    /// Filter by workspace name
    #[arg(long)]
    pub workspace: Option<String>,

    /// Maximum number of entries to show  [default: 20]
    #[arg(long, default_value = "20")]
    pub limit: usize,
}

// ── template ───────────────────────────────────────────────────────────────────

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
    /// List all available templates (builtin + user)
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

pub fn run(args: DocumentArgs) -> Result<()> {
    match args.subcommand {
        DocumentSubcommand::Create(a) => run_create(a),
        DocumentSubcommand::Update(a) => run_update(a),
        DocumentSubcommand::List(a) => run_list(a),
        DocumentSubcommand::Template(a) => run_template(a),
    }
}

fn run_create(args: CreateArgs) -> Result<()> {
    let format = DocumentFormat::parse(&args.r#type)?;

    let content = resolve_content(args.content.as_deref(), args.content_file.as_deref())?;

    // Resolve workspace/tenant/project/repository from args or env.
    let (workspace_name, tenant, project, repository) = resolve_scope(args.workspace.as_deref());

    let output = match args.output {
        Some(p) => p,
        None => {
            let slug = slugify_title(&args.title);
            let scope_dir =
                data_paths::documents_scope_dir(&workspace_name, &tenant, &project, &repository);
            scope_dir.join(format!("{slug}.{}", format.extension()))
        }
    };

    if output.exists() && !args.force {
        bail!(
            "output file already exists: {}. Use --force to overwrite.",
            output.display()
        );
    }

    let vars = parse_vars(&args.vars)?;

    let req = GenerateRequest {
        title: args.title.clone(),
        format,
        content,
        template: args.template,
        output,
        vars,
        workspace: Some(workspace_name),
        tenant: Some(tenant),
        project: Some(project),
        repository: Some(repository),
        scope: None,
        skip_index: false,
    };

    let result = document::generate(&req)
        .with_context(|| format!("failed to generate {} document", req.title))?;

    println!(
        "  {} → {} ({} bytes)",
        req.title,
        result.output.display(),
        result.bytes
    );
    println!("  source: {}", result.source.display());

    if args.open {
        let dir = result.output.parent().unwrap_or(&result.output);
        open::that(dir).with_context(|| format!("failed to open directory {}", dir.display()))?;
    }

    Ok(())
}

fn run_update(args: UpdateArgs) -> Result<()> {
    let (workspace_name, entry) = find_entry(&args.document)
        .with_context(|| format!("document '{}' not found in the index", args.document))?;

    let content = resolve_content(args.content.as_deref(), args.content_file.as_deref())?;

    // Backup existing source file.
    if entry.source_path.exists() {
        let backup = PathBuf::from(format!("{}.backup", entry.source_path.display()));
        fs::copy(&entry.source_path, &backup)
            .with_context(|| format!("failed to backup source to {}", backup.display()))?;
        println!("  backup: {}", backup.display());
    }

    // Write updated source.
    fs::write(&entry.source_path, &content)
        .with_context(|| format!("failed to write source {}", entry.source_path.display()))?;

    let format = DocumentFormat::parse(&entry.format)?;
    let vars = {
        let mut v = entry.vars.clone();
        v.extend(parse_vars(&args.vars)?);
        v
    };

    let req = GenerateRequest {
        title: entry.title.clone(),
        format,
        content,
        template: args.template.or_else(|| entry.template.clone()),
        output: entry.output_path.clone(),
        vars,
        workspace: Some(entry.workspace.clone()),
        tenant: Some(entry.tenant.clone()),
        project: Some(entry.project.clone()),
        repository: Some(entry.repository.clone()),
        scope: None,
        // skip_index=true: we update the existing entry manually below.
        skip_index: true,
    };

    let result = document::generate(&req)
        .with_context(|| format!("failed to regenerate document {}", entry.id))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let updated_entry = DocumentEntry {
        source_path: result.source.clone(),
        updated_at: now,
        ..entry.clone()
    };
    let _ = update_stored_entry(&workspace_name, &entry.id, updated_entry);

    println!(
        "  {} → {} ({} bytes)",
        entry.id,
        result.output.display(),
        result.bytes
    );

    if args.open {
        let dir = result.output.parent().unwrap_or(&result.output);
        open::that(dir).with_context(|| format!("failed to open directory {}", dir.display()))?;
    }

    Ok(())
}

fn run_list(args: ListArgs) -> Result<()> {
    let entries: Vec<DocumentEntry> = match &args.workspace {
        Some(ws) => load_all_entries(ws),
        None => load_all_entries_global(),
    };

    if entries.is_empty() {
        println!("No documents found.");
        return Ok(());
    }

    let shown: Vec<_> = entries.iter().rev().take(args.limit).collect();
    println!("{:<12} {:<10} {:<30} PATH", "ID", "FORMAT", "TITLE");
    println!("{}", "─".repeat(90));
    for e in shown.iter().rev() {
        println!(
            "{:<12} {:<10} {:<30} {}",
            e.id,
            e.format,
            truncate(&e.title, 30),
            e.output_path.display()
        );
    }

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
                println!("No templates available.");
                return Ok(());
            }
            println!("{:<22} {:<10} DESCRIPTION", "NAME", "SOURCE");
            println!("{}", "─".repeat(72));
            for (source, meta) in &templates {
                let src_label = match source {
                    TemplateSource::Builtin => "builtin".to_string(),
                    TemplateSource::User(_) => "user".to_string(),
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

/// Extract workspace_name, tenant, project, repository from args or env vars.
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
        (None, None) => {
            eprintln!(
                "[orbit document] warning: no content provided. Use --content or --content-file."
            );
            Ok(String::new())
        }
    }
}

fn slugify_title(title: &str) -> String {
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
