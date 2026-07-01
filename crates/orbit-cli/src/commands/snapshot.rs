use anyhow::{Context, Result, bail};
use clap::Args;
use orbit_core::{context::OrbitScope, user_config::UserConfig};
use orbit_engine::resolver;
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

/// Files each engine generates when you run its init/context command.
const ENGINE_CONTEXT_FILES: &[(&str, &str)] = &[
    ("claude", "CLAUDE.md"),
    ("opencode", "AGENTS.md"),
    ("gemini", "GEMINI.md"),
];

/// Fallback candidates tried in order when no engine-specific file is found.
const FALLBACK_FILES: &[&str] = &["CLAUDE.md", "AGENTS.md", "GEMINI.md", "CONTEXT.md"];

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    /// Source file to sync (default: auto-detect CLAUDE.md / AGENTS.md / GEMINI.md in cwd)
    #[arg(long, short)]
    pub file: Option<PathBuf>,

    /// Read content from stdin instead of a file
    #[arg(long)]
    pub stdin: bool,

    /// Override output path in governance repo
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Print resolved paths without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: SnapshotArgs) -> Result<()> {
    let scope = resolver::resolve_from_cwd()
        .context("could not resolve scope from cwd — are you inside a workspace?")?;

    let dest = args
        .output
        .clone()
        .unwrap_or_else(|| governance_path(&scope));

    // ── read source content ───────────────────────────────────────────────────
    let (content, source_label) = if args.stdin {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        (buf, "stdin".to_string())
    } else if let Some(ref path) = args.file {
        let content =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        (content, path.display().to_string())
    } else {
        // Auto-detect engine context file in cwd
        let cwd = std::env::current_dir()?;
        let user_cfg = UserConfig::load();
        let detected = detect_context_file(&cwd, &user_cfg.engine.default);
        match detected {
            Some(path) => {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("cannot read {}", path.display()))?;
                (content, path.display().to_string())
            }
            None => {
                let tried: Vec<_> = FALLBACK_FILES
                    .iter()
                    .map(|f| cwd.join(f).display().to_string())
                    .collect();
                bail!(
                    "no context file found in current directory\n\n  tried:\n{}\n\n  Generate one first:\n    claude → /init\n    opencode → /init\n    gemini → /init\n\n  or pass --file <path> or --stdin",
                    tried
                        .iter()
                        .map(|p| format!("    {p}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    };

    if args.dry_run {
        println!("  source → {source_label}");
        println!("  dest   → {}", dest.display());
        println!("  scope  → {}", scope_label(&scope));
        return Ok(());
    }

    // ── write to governance layer ─────────────────────────────────────────────
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dest, &content)?;

    println!("  synced {} → {}", source_label, dest.display());
    println!("  scope: {}", scope_label(&scope));

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn detect_context_file(cwd: &Path, default_engine: &str) -> Option<PathBuf> {
    // Try engine-specific file first
    for (engine, filename) in ENGINE_CONTEXT_FILES {
        if engine.eq_ignore_ascii_case(default_engine) {
            let candidate = cwd.join(filename);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    // Then try all known files in order
    for filename in FALLBACK_FILES {
        let candidate = cwd.join(filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn governance_path(scope: &OrbitScope) -> PathBuf {
    // Target filename matches the source engine convention — store as context.md
    // in the appropriate source-of-truth layer.
    let sot_root = if !scope.repository.is_empty() {
        scope
            .global_ai_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("repositories")
            .join(&scope.repository)
            .join("source-of-truth")
    } else if !scope.project.is_empty() {
        scope
            .global_ai_root
            .join("tenants")
            .join(&scope.tenant)
            .join("projects")
            .join(&scope.project)
            .join("source-of-truth")
    } else if !scope.tenant.is_empty() {
        scope
            .global_ai_root
            .join("tenants")
            .join(&scope.tenant)
            .join("source-of-truth")
    } else {
        scope.global_ai_root.clone()
    };

    sot_root.join("context.md")
}

fn scope_label(scope: &OrbitScope) -> String {
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
    if parts.is_empty() {
        "global".to_string()
    } else {
        parts.join(" / ")
    }
}
