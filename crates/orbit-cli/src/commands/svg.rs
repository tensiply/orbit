use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use orbit_core::{
    data_paths,
    svg::{
        self, SvgBackend, SvgEntry, SvgGenerateRequest, TemplateSource, add_user_template,
        find_entry, list_templates, load_all_entries, load_all_entries_global, next_id,
        remove_user_template, resolve_template, update_stored_entry,
    },
};
use std::{collections::HashMap, fs, path::PathBuf};

// ── top-level args ─────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct SvgArgs {
    #[command(subcommand)]
    pub subcommand: SvgSubcommand,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum SvgSubcommand {
    /// Generate an SVG from a template or raw SVG content
    Create(CreateArgs),
    /// Edit an existing SVG: backup source, apply new content, regenerate
    Update(UpdateArgs),
    /// List generated SVGs tracked in the index
    List(ListArgs),
    /// Open a generated SVG by ID or path
    Open(OpenArgs),
    /// Manage SVG templates
    Template(TemplateArgs),
}

// ── create ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct CreateArgs {
    /// SVG title (required)
    #[arg(long, short = 't')]
    pub title: String,

    /// Description or raw SVG content — stored as .txt alongside the output
    #[arg(long, short = 'c', conflicts_with = "content_file")]
    pub content: Option<String>,

    /// Path to a file containing the description or raw SVG content
    #[arg(long, short = 'f')]
    pub content_file: Option<PathBuf>,

    /// Generation backend: template (minijinja render) or raw (write content verbatim)
    #[arg(long, short = 'b', default_value = "template")]
    pub backend: String,

    /// Template name (for template backend; overrides rule default)
    #[arg(long)]
    pub template: Option<String>,

    /// Template variable: KEY=VALUE (repeatable)
    #[arg(long = "var", short = 'v', value_name = "KEY=VALUE")]
    pub vars: Vec<String>,

    /// Output path. Defaults to `~/.orbit/files/svgs/{scope}/{slug}.svg`.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Skip backup when the output file already exists (default: backup to .bk)
    #[arg(long)]
    pub force: bool,

    /// Replace an existing SVG instead of creating a new one.
    /// Without a value, replaces the most recently created SVG in the workspace.
    /// With a value (SVG-ID or path), replaces that specific SVG.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    pub replace: Option<String>,

    /// Workspace name override (auto-detected from AI_WORKSPACE_ROOT)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ── update ────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct UpdateArgs {
    /// SVG ID (e.g., SVG-000001) or path to the output file
    pub svg: String,

    /// New description or raw SVG content
    #[arg(long, short = 'c', conflicts_with = "content_file")]
    pub content: Option<String>,

    /// Path to a file containing the new content
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

    /// Maximum number of entries to show
    #[arg(long, default_value = "20")]
    pub limit: usize,
}

// ── open ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct OpenArgs {
    /// SVG ID (e.g., SVG-000001) or path to the output file
    pub svg: String,
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
        /// Path to the SVG template file
        path: PathBuf,
    },
    /// List all available templates (builtin + user + workspace)
    List,
    /// Remove a user-defined template (builtins cannot be removed)
    Remove {
        /// Template name (file stem, without .svg)
        name: String,
    },
    /// Show the raw SVG source of a template
    Show {
        /// Template name (file stem, without .svg)
        name: String,
    },
}

// ── run ───────────────────────────────────────────────────────────────────────

pub fn run(args: SvgArgs) -> Result<()> {
    match args.subcommand {
        SvgSubcommand::Create(a) => run_create(a),
        SvgSubcommand::Update(a) => run_update(a),
        SvgSubcommand::List(a) => run_list(a),
        SvgSubcommand::Open(a) => run_open(a),
        SvgSubcommand::Template(a) => run_template(a),
    }
}

fn run_create(args: CreateArgs) -> Result<()> {
    let backend = SvgBackend::parse(&args.backend)?;
    let content = resolve_content(args.content.as_deref(), args.content_file.as_deref())?;
    let (workspace_name, tenant, project, repository) = resolve_scope(args.workspace.as_deref());

    let replace_entry: Option<SvgEntry> = if let Some(ref key) = args.replace {
        if key.is_empty() {
            last_entry(&workspace_name)
                .with_context(|| "no SVGs found in this workspace to replace")?
                .into()
        } else {
            let (_, e) =
                find_entry(key).with_context(|| format!("SVG '{key}' not found in the index"))?;
            Some(e)
        }
    } else {
        None
    };

    let (output, alloc_id, existing): (PathBuf, String, Option<SvgEntry>) =
        if let Some(ref e) = replace_entry {
            (e.output_path.clone(), e.id.clone(), replace_entry)
        } else if let Some(p) = args.output {
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
                let scope_dir =
                    data_paths::svgs_scope_dir(&workspace_name, &tenant, &project, &repository);
                let path = scope_dir.join(format!("{id}-{slug}.svg"));
                (path, id, None)
            }
        };

    let skip_backup = args.force || args.replace.is_some();
    if output.exists() && !skip_backup {
        let backup = PathBuf::from(format!("{}.bk", output.display()));
        if let Err(e) = fs::copy(&output, &backup) {
            eprintln!("[orbit svg] warn: could not backup existing file: {e}");
        }
    }

    let vars = parse_vars(&args.vars)?;

    let req = SvgGenerateRequest {
        title: args.title.clone(),
        description: content,
        backend,
        template: args.template,
        vars,
        output,
        workspace: Some(workspace_name.clone()),
        tenant: Some(tenant),
        project: Some(project),
        repository: Some(repository),
        id: Some(alloc_id),
        skip_index: existing.is_some(),
    };

    let result =
        svg::generate(&req).with_context(|| format!("failed to generate SVG '{}'", req.title))?;

    if let Some(entry) = existing {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated = SvgEntry {
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
        eprintln!("[orbit svg] warn: could not open directory: {e}");
    }

    Ok(())
}

fn run_update(args: UpdateArgs) -> Result<()> {
    let (workspace_name, entry) = find_entry(&args.svg)
        .with_context(|| format!("SVG '{}' not found in the index", args.svg))?;

    let content = resolve_content(args.content.as_deref(), args.content_file.as_deref())?;

    if entry.source_path.exists() {
        let backup = PathBuf::from(format!("{}.backup", entry.source_path.display()));
        fs::copy(&entry.source_path, &backup)
            .with_context(|| format!("failed to backup source to {}", backup.display()))?;
        println!("  backup: {}", backup.display());
    }

    fs::write(&entry.source_path, &content)
        .with_context(|| format!("failed to write source {}", entry.source_path.display()))?;

    let backend = SvgBackend::parse(&entry.backend)?;
    let vars = {
        let mut v = entry.vars.clone();
        v.extend(parse_vars(&args.vars)?);
        v
    };

    let req = SvgGenerateRequest {
        title: entry.title.clone(),
        description: content,
        backend,
        template: args.template.or_else(|| entry.template.clone()),
        vars,
        output: entry.output_path.clone(),
        workspace: Some(entry.workspace.clone()),
        tenant: Some(entry.tenant.clone()),
        project: Some(entry.project.clone()),
        repository: Some(entry.repository.clone()),
        id: None,
        skip_index: true,
    };

    let result =
        svg::generate(&req).with_context(|| format!("failed to regenerate SVG {}", entry.id))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let updated_entry = SvgEntry {
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
        if let Err(e) = open::that(dir) {
            eprintln!("[orbit svg] warn: could not open directory: {e}");
        }
    }

    Ok(())
}

fn run_list(args: ListArgs) -> Result<()> {
    let entries: Vec<SvgEntry> = match &args.workspace {
        Some(ws) => load_all_entries(ws),
        None => load_all_entries_global(),
    };

    if entries.is_empty() {
        println!("No SVGs found.");
        return Ok(());
    }

    let shown: Vec<_> = entries.iter().rev().take(args.limit).collect();
    println!(
        "{:<12} {:<12} {:<10} {:<28} PATH",
        "ID", "WORKSPACE", "BACKEND", "TITLE"
    );
    println!("{}", "─".repeat(100));
    for e in shown.iter().rev() {
        let ws_display = if e.workspace.is_empty() {
            "—".to_string()
        } else {
            truncate(&e.workspace, 12)
        };
        println!(
            "{:<12} {:<12} {:<10} {:<28} {}",
            e.id,
            ws_display,
            e.backend,
            truncate(&e.title, 28),
            e.output_path.display()
        );
    }

    Ok(())
}

fn run_open(args: OpenArgs) -> Result<()> {
    let (_ws, entry) = find_entry(&args.svg)
        .with_context(|| format!("SVG '{}' not found in the index", args.svg))?;

    if !entry.output_path.exists() {
        anyhow::bail!(
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
                println!("No SVG templates available.");
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

fn find_by_output(output: &std::path::Path, workspace_name: &str) -> Option<SvgEntry> {
    let target = output.to_string_lossy();
    load_all_entries(workspace_name)
        .into_iter()
        .find(|e| e.output_path.to_string_lossy() == target)
}

fn find_by_title(title: &str, workspace_name: &str) -> Option<SvgEntry> {
    load_all_entries(workspace_name)
        .into_iter()
        .find(|e| e.title == title)
}

fn last_entry(workspace_name: &str) -> Option<SvgEntry> {
    load_all_entries(workspace_name).into_iter().last()
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
