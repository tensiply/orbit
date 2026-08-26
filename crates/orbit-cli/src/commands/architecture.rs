use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::{
    architecture::{
        CatalogEntity, CatalogLoadResult, EntityKind, filter_by_project, load_catalog,
    },
    resolver::{self, ResolveArgs, resolve_from_cwd},
};

use crate::output::truncate_desc;

#[derive(Debug, Args)]
pub struct ArchitectureArgs {
    /// Workspace name (case-insensitive). Omit to auto-detect from current directory.
    pub workspace: Option<String>,
    /// Tenant name within the workspace
    pub tenant: Option<String>,
    /// Project name — filters entities by matching project tag
    pub project: Option<String>,
    /// Repository name
    pub repository: Option<String>,

    #[command(subcommand)]
    pub command: Option<ArchSubcommand>,
}

#[derive(Debug, Subcommand)]
pub enum ArchSubcommand {
    /// Show a structured architecture summary grouped by entity kind (default)
    Show,
    /// List all entities in a table, optionally filtered by kind
    List {
        /// Filter by entity kind: service, database, integration, infrastructure, api, pipeline, secret, iam, team
        #[arg(long, short)]
        kind: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Validate entity fields and optional cross-references
    Validate {
        /// Also check that IDs referenced in depends_on and used_by exist in the catalog
        #[arg(long)]
        refs: bool,
    },
    /// Export the catalog view to stdout
    Export {
        /// Output format: md or json
        #[arg(long, short, default_value = "md")]
        format: String,
    },
}

pub fn run(args: ArchitectureArgs) -> Result<()> {
    let scope = if args.workspace.is_none() {
        resolve_from_cwd().or_else(|_| {
            resolver::resolve(ResolveArgs::default())
        })?
    } else {
        resolver::resolve(ResolveArgs {
            workspace: args.workspace.clone(),
            tenant: args.tenant.clone(),
            project: args.project.clone(),
            repository: args.repository.clone(),
        })?
    };

    if scope.tenant.is_empty() {
        bail!(
            "Architecture requires a tenant scope.\n\
             Usage: orbit architecture WORKSPACE TENANT [PROJECT [REPO]]\n\
             Example: orbit architecture befra jafra"
        );
    }

    let result = load_catalog(&scope.tenant_dir);

    let project_filter = if scope.project.is_empty() {
        None
    } else {
        Some(scope.project.as_str())
    };

    match args.command.unwrap_or(ArchSubcommand::Show) {
        ArchSubcommand::Show => show(&result, project_filter, &scope.tenant),
        ArchSubcommand::List { kind, json } => list(&result, project_filter, kind.as_deref(), json),
        ArchSubcommand::Validate { refs } => validate(&result, refs),
        ArchSubcommand::Export { format } => export(&result, project_filter, &format),
    }
}

// ── show ──────────────────────────────────────────────────────────────────────

fn show(result: &CatalogLoadResult, project: Option<&str>, tenant: &str) -> Result<()> {
    let entities = filter_entities(result, project);

    let workspace_hint = result
        .tenant_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    println!();
    if let Some(proj) = project {
        println!("  \x1b[1marchitecture\x1b[0m  {tenant}  /  {proj}  \x1b[2m({workspace_hint})\x1b[0m");
    } else {
        println!("  \x1b[1marchitecture\x1b[0m  {tenant}  \x1b[2m({workspace_hint})\x1b[0m");
    }
    println!();

    // Summary counts per kind
    let ordered_kinds = [
        EntityKind::Service,
        EntityKind::Database,
        EntityKind::Integration,
        EntityKind::Infrastructure,
        EntityKind::Api,
        EntityKind::Pipeline,
        EntityKind::SecretGroup,
        EntityKind::Iam,
        EntityKind::Team,
    ];

    let count_w = 4usize;
    for kind in &ordered_kinds {
        let count = entities.iter().filter(|e| &e.kind == kind).count();
        if count == 0 {
            continue;
        }
        let crits = result.criticality_counts(kind);
        let crit_str = if crits.is_empty() {
            String::new()
        } else {
            crits
                .iter()
                .map(|(k, c)| format!("{k}:{c}"))
                .collect::<Vec<_>>()
                .join("  ")
        };
        println!(
            "  {:<16}  {:>count_w$}   \x1b[2m{crit_str}\x1b[0m",
            kind.display_name(),
            count,
            count_w = count_w,
        );
    }

    // Detail sections per kind
    for kind in &ordered_kinds {
        let kind_entities: Vec<&&CatalogEntity> =
            entities.iter().filter(|e| &e.kind == kind).collect();
        if kind_entities.is_empty() {
            continue;
        }

        println!();
        println!(
            "  \x1b[2m── {} {}\x1b[0m",
            kind.display_name(),
            "─".repeat(50usize.saturating_sub(kind.display_name().len()))
        );

        let id_w = kind_entities
            .iter()
            .map(|e| e.id.len())
            .max()
            .unwrap_or(8)
            .max(8)
            .min(40);

        for e in &kind_entities {
            let crit_color = match e.criticality.as_deref() {
                Some("critical") => "\x1b[31m",
                Some("high") => "\x1b[33m",
                Some("medium") => "\x1b[36m",
                _ => "\x1b[2m",
            };
            let crit = e
                .criticality
                .as_deref()
                .map(|c| format!("{crit_color}{c}\x1b[0m"))
                .unwrap_or_default();
            let lc = e
                .lifecycle
                .as_deref()
                .map(|l| format!("\x1b[2m{l}\x1b[0m"))
                .unwrap_or_default();
            let summary = e
                .summary()
                .map(|s| format!("  \x1b[2m{s}\x1b[0m"))
                .unwrap_or_default();

            println!(
                "  \x1b[32m●\x1b[0m  {:<id_w$}  {crit}  {lc}{summary}",
                truncate_desc(&e.id, id_w),
                id_w = id_w,
            );
        }
    }

    println!();
    let total = entities.len();
    let errs = result.errors.len();
    let hint = if !result.tenant_dir.exists() {
        format!(
            "  \x1b[33mNo catalog found at {}\x1b[0m",
            result.tenant_dir.join("source-of-truth").join("catalog").display()
        )
    } else {
        String::new()
    };
    if !hint.is_empty() {
        println!("{hint}");
    }
    let err_note = if errs > 0 {
        format!("  ·  \x1b[31m{errs} parse error(s)\x1b[0m")
    } else {
        String::new()
    };
    println!("  {total} entities{err_note}");
    println!();
    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────────

fn list(
    result: &CatalogLoadResult,
    project: Option<&str>,
    kind_filter: Option<&str>,
    as_json: bool,
) -> Result<()> {
    let mut entities = filter_entities(result, project);

    if let Some(kf) = kind_filter {
        let kf_lc = kf.to_lowercase();
        entities.retain(|e| {
            e.kind.folder_name().contains(&kf_lc)
                || e.kind.display_name().to_lowercase().contains(&kf_lc)
        });
    }

    if as_json {
        println!("{}", serde_json::to_string_pretty(&entities)?);
        return Ok(());
    }

    println!();
    let id_w = entities
        .iter()
        .map(|e| e.id.len())
        .max()
        .unwrap_or(8)
        .max(8)
        .min(40);
    let kind_w = 14usize;
    let name_w = 36usize;
    let crit_w = 8usize;
    let lc_w = 11usize;

    println!(
        "  \x1b[2m  {:<id_w$}  {:<kind_w$}  {:<name_w$}  {:<crit_w$}  {:<lc_w$}\x1b[0m",
        "id",
        "kind",
        "name",
        "criticality",
        "lifecycle",
        id_w = id_w,
        kind_w = kind_w,
        name_w = name_w,
        crit_w = crit_w,
        lc_w = lc_w,
    );
    let sep = 4 + id_w + 2 + kind_w + 2 + name_w + 2 + crit_w + 2 + lc_w;
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep));

    for e in &entities {
        println!(
            "    {:<id_w$}  \x1b[2m{:<kind_w$}\x1b[0m  {:<name_w$}  \x1b[2m{:<crit_w$}  {:<lc_w$}\x1b[0m",
            truncate_desc(&e.id, id_w),
            truncate_desc(e.kind.display_name(), kind_w),
            truncate_desc(&e.name, name_w),
            e.criticality.as_deref().unwrap_or("—"),
            e.lifecycle.as_deref().unwrap_or("—"),
            id_w = id_w,
            kind_w = kind_w,
            name_w = name_w,
            crit_w = crit_w,
            lc_w = lc_w,
        );
    }
    println!("  \x1b[2m{}\x1b[0m", "─".repeat(sep));
    println!();
    println!("  {} entities", entities.len());
    println!();
    Ok(())
}

// ── validate ──────────────────────────────────────────────────────────────────

fn validate(result: &CatalogLoadResult, check_refs: bool) -> Result<()> {
    println!();
    let mut issues: Vec<(String, String)> = Vec::new();

    for e in &result.entities {
        if e.id.is_empty() {
            issues.push((e.name.clone(), "missing required field: id".to_string()));
        }
        if e.name.is_empty() {
            issues.push((e.id.clone(), "missing required field: name".to_string()));
        }
        if e.criticality.is_none() {
            issues.push((e.id.clone(), "missing recommended field: criticality".to_string()));
        }
        if e.lifecycle.is_none() {
            issues.push((e.id.clone(), "missing recommended field: lifecycle".to_string()));
        }
    }

    // Cross-reference validation
    if check_refs {
        let all_ids: std::collections::HashSet<&str> =
            result.entities.iter().map(|e| e.id.as_str()).collect();

        for e in &result.entities {
            // Check depends_on.services
            if let Some(deps) = &e.depends_on {
                if let Some(services) = deps.get("services").and_then(|v| v.as_sequence()) {
                    for svc in services {
                        if let Some(id) = svc.as_str() {
                            if !all_ids.contains(id) {
                                issues.push((
                                    e.id.clone(),
                                    format!("depends_on.services references unknown id: {id}"),
                                ));
                            }
                        }
                    }
                }
            }
            // Check used_by.services
            if let Some(used) = &e.used_by {
                if let Some(services) = used.get("services").and_then(|v| v.as_sequence()) {
                    for svc in services {
                        if let Some(id) = svc.as_str() {
                            if !all_ids.contains(id) {
                                issues.push((
                                    e.id.clone(),
                                    format!("used_by.services references unknown id: {id}"),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Parse errors
    for (path, err) in &result.errors {
        issues.push((
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string(),
            format!("parse error: {err}"),
        ));
    }

    if issues.is_empty() {
        println!("  \x1b[32m✓\x1b[0m  {} entities — no issues found", result.entities.len());
    } else {
        println!(
            "  \x1b[31m✗\x1b[0m  {} entities — {} issue(s) found\n",
            result.entities.len(),
            issues.len()
        );
        for (id, msg) in &issues {
            println!("  \x1b[2m{id}\x1b[0m  {msg}");
        }
    }
    println!();
    Ok(())
}

// ── export ────────────────────────────────────────────────────────────────────

fn export(result: &CatalogLoadResult, project: Option<&str>, format: &str) -> Result<()> {
    let entities = filter_entities(result, project);

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&entities)?);
        }
        "md" | _ => {
            let ordered_kinds = [
                EntityKind::Service,
                EntityKind::Database,
                EntityKind::Integration,
                EntityKind::Infrastructure,
                EntityKind::Api,
                EntityKind::Pipeline,
                EntityKind::SecretGroup,
                EntityKind::Iam,
                EntityKind::Team,
            ];

            println!("# Architecture Catalog\n");

            for kind in &ordered_kinds {
                let kind_entities: Vec<&&CatalogEntity> =
                    entities.iter().filter(|e| &e.kind == kind).collect();
                if kind_entities.is_empty() {
                    continue;
                }

                println!("## {}\n", kind.display_name());
                println!("| ID | Name | Criticality | Lifecycle |");
                println!("|---|---|---|---|");
                for e in &kind_entities {
                    println!(
                        "| {} | {} | {} | {} |",
                        e.id,
                        e.name,
                        e.criticality.as_deref().unwrap_or("—"),
                        e.lifecycle.as_deref().unwrap_or("—"),
                    );
                }
                println!();
            }
        }
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn filter_entities<'a>(result: &'a CatalogLoadResult, project: Option<&str>) -> Vec<&'a CatalogEntity> {
    if let Some(proj) = project {
        filter_by_project(&result.entities, proj)
    } else {
        result.entities.iter().collect()
    }
}
