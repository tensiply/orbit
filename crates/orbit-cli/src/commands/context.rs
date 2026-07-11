use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::{engine::Engine, user_config::UserConfig};
use orbit_engine::{config, resolver};
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: Option<ContextCommand>,

    /// Engine to inspect (default: reads from config)
    #[arg(long, short)]
    pub engine: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// Show all context layers for the current scope (default)
    Show {
        /// Engine to inspect (default: reads from config)
        #[arg(long, short)]
        engine: Option<String>,
    },
    /// Print only the instruction files that would be loaded, one per line
    Which {
        /// Engine to inspect
        #[arg(long, short)]
        engine: Option<String>,
    },
    /// Dump the resolved scope for the current directory
    Scope,
}

pub fn run(args: ContextArgs) -> Result<()> {
    match args.command {
        None => cmd_show(args.engine.as_deref()),
        Some(ContextCommand::Show { engine }) => {
            cmd_show(args.engine.or(engine).as_deref())
        }
        Some(ContextCommand::Which { engine }) => {
            cmd_which(args.engine.or(engine).as_deref())
        }
        Some(ContextCommand::Scope) => cmd_scope(),
    }
}

// ── show ──────────────────────────────────────────────────────────────────────

fn cmd_show(engine_override: Option<&str>) -> Result<()> {
    let scope = resolver::resolve_from_cwd()
        .unwrap_or_else(|_| resolver::resolve(Default::default()).unwrap_or_default());

    let engine = resolve_engine(engine_override)?;
    let (merged, report) = config::inspect(&scope, engine)?;

    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));

    println!();
    println!("  \x1b[1mcontext\x1b[0m  ·  engine: {engine}");
    println!();

    // ── scope ─────────────────────────────────────────────────────────────────
    println!("  \x1b[2mscope\x1b[0m");
    let label = scope_label_str(&scope);
    println!("    {}", if label.is_empty() { "global".to_string() } else { label });
    println!();

    // ── config layers ─────────────────────────────────────────────────────────
    println!("  \x1b[2mconfig layers\x1b[0m");
    for entry in &report.config_layers {
        let icon = if entry.exists { "\x1b[32m✓\x1b[0m" } else { "\x1b[2m○\x1b[0m" };
        let path = shorten(&home, &entry.path);
        println!(
            "    {icon}  {:<22}  \x1b[2m{}\x1b[0m",
            entry.label,
            path.display()
        );
    }
    println!();

    // ── instruction files ─────────────────────────────────────────────────────
    let loaded_count = report.instructions.iter().filter(|(_, ok)| *ok).count();
    let missing_count = report.instructions.len() - loaded_count;
    println!(
        "  \x1b[2minstructions\x1b[0m  ({} loaded{})",
        loaded_count,
        if missing_count > 0 {
            format!(", {missing_count} not found")
        } else {
            String::new()
        },
    );
    for (path, ok) in &report.instructions {
        let icon = if *ok { "\x1b[32m✓\x1b[0m" } else { "\x1b[33m!\x1b[0m" };
        println!("    {icon}  {}", shorten(&home, path).display());
    }
    if report.instructions.is_empty() {
        println!("    \x1b[2m(none)\x1b[0m");
    }
    println!();

    // ── MCP servers ───────────────────────────────────────────────────────────
    if !merged.mcp.is_empty() {
        println!("  \x1b[2mMCP servers\x1b[0m  ({})", merged.mcp.len());
        let mut names: Vec<_> = merged.mcp.keys().collect();
        names.sort();
        for name in names {
            let server = &merged.mcp[name];
            let cmd = server.command.join(" ");
            println!("    \x1b[32m●\x1b[0m  {name}  \x1b[2m{cmd}\x1b[0m");
        }
        println!();
    }

    // ── env vars ──────────────────────────────────────────────────────────────
    if !report.env_vars.is_empty() {
        println!("  \x1b[2menv vars\x1b[0m  ({})", report.env_vars.len());
        for (k, v) in &report.env_vars {
            let short_v: String = v.chars().take(60).collect();
            let ellipsis = if v.len() > 60 { "…" } else { "" };
            println!("    {k}={short_v}{ellipsis}");
        }
        println!();
    }

    // ── extra config ──────────────────────────────────────────────────────────
    if !merged.extra.is_empty() {
        println!("  \x1b[2mextra config\x1b[0m");
        for (k, v) in &merged.extra {
            println!("    {k}: {v}");
        }
        println!();
    }

    Ok(())
}

// ── which ─────────────────────────────────────────────────────────────────────

fn cmd_which(engine_override: Option<&str>) -> Result<()> {
    let scope = resolver::resolve_from_cwd()
        .unwrap_or_else(|_| resolver::resolve(Default::default()).unwrap_or_default());

    let engine = resolve_engine(engine_override)?;
    let (_merged, report) = config::inspect(&scope, engine)?;

    for (path, ok) in &report.instructions {
        if *ok {
            println!("{}", path.display());
        }
    }

    Ok(())
}

// ── scope ─────────────────────────────────────────────────────────────────────

fn cmd_scope() -> Result<()> {
    let scope = resolver::resolve_from_cwd()
        .unwrap_or_else(|_| resolver::resolve(Default::default()).unwrap_or_default());

    println!();
    println!("  \x1b[1mresolved scope\x1b[0m");
    println!();
    println!("    tenant     {}", if scope.tenant.is_empty() { "(none)".to_string() } else { scope.tenant.clone() });
    println!("    project    {}", if scope.project.is_empty() { "(none)".to_string() } else { scope.project.clone() });
    println!("    repository {}", if scope.repository.is_empty() { "(none)".to_string() } else { scope.repository.clone() });
    println!();
    println!("    global_ai_root  {}", scope.global_ai_root.display());
    println!("    workspace_root  {}", scope.workspace_root.display());
    println!("    work_dir        {}", scope.work_dir.display());
    println!();

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn resolve_engine(engine_override: Option<&str>) -> Result<Engine> {
    match engine_override {
        Some(s) => s
            .parse::<Engine>()
            .map_err(|e| anyhow::anyhow!("{e}")),
        None => Ok(UserConfig::load()
            .engine
            .default
            .parse::<Engine>()
            .unwrap_or(Engine::Claude)),
    }
}

fn scope_label_str(scope: &orbit_core::context::OrbitScope) -> String {
    let mut parts: Vec<&str> = vec![];
    if !scope.tenant.is_empty() {
        parts.push(&scope.tenant);
    }
    if !scope.project.is_empty() {
        parts.push(&scope.project);
    }
    if !scope.repository.is_empty() {
        parts.push(&scope.repository);
    }
    parts.join(" / ")
}

fn shorten(home: &Path, p: &Path) -> PathBuf {
    if let Ok(rel) = p.strip_prefix(home) {
        PathBuf::from("~").join(rel)
    } else {
        p.to_path_buf()
    }
}
