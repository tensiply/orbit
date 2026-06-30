use anyhow::{Result, bail};
use clap::Args;
use orbit_core::user_config::UserConfig;
use std::process::Command;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// URL of the governance git repository (omit with --scaffold for local-only setup)
    pub governance_url: Option<String>,

    /// Branch to clone [default: main]
    #[arg(long, default_value = "main")]
    pub branch: String,

    /// Override AI root destination (default: from config)
    #[arg(long)]
    pub ai_root: Option<std::path::PathBuf>,

    /// Create a local-only AI root scaffold without cloning a governance repo
    #[arg(long)]
    pub scaffold: bool,
}

pub async fn run(args: InitArgs) -> Result<()> {
    let cfg = UserConfig::load();
    let ai_root = args.ai_root.unwrap_or_else(|| cfg.ai_root_expanded());

    if args.scaffold {
        return scaffold(&ai_root);
    }

    let governance_url = args.governance_url.ok_or_else(|| {
        anyhow::anyhow!(
            "Provide a governance repository URL, or use --scaffold for a local-only setup.\n\
             Example: orbit init git@github.com:myorg/ai-governance.git\n\
             Example: orbit init --scaffold"
        )
    })?;

    if ai_root.is_dir() && ai_root.join(".git").is_dir() {
        bail!(
            "AI root already initialised: {}\nRun `orbit update` to pull the latest changes.",
            ai_root.display()
        );
    }

    if ai_root.exists() && !is_empty_dir(&ai_root) {
        bail!(
            "{} already exists and is not empty.\n\
             Remove it first or use --ai-root to choose a different path.",
            ai_root.display()
        );
    }

    println!("Cloning {} → {}", governance_url, ai_root.display());

    let status = Command::new("git")
        .args([
            "clone",
            "--branch",
            &args.branch,
            "--single-branch",
            &governance_url,
            &ai_root.to_string_lossy(),
        ])
        .status()?;

    if !status.success() {
        bail!("git clone failed — check the URL and your network/SSH access");
    }

    println!();
    println!("  Governance repo cloned to {}", ai_root.display());
    println!("  Run `orbit launch` to start a session.");
    println!();

    Ok(())
}

fn scaffold(ai_root: &std::path::Path) -> Result<()> {
    if ai_root.is_dir() && ai_root.join(".git").is_dir() {
        bail!(
            "AI root already initialised: {}\nRun `orbit update` to pull the latest changes.",
            ai_root.display()
        );
    }

    if ai_root.exists() && !is_empty_dir(ai_root) {
        bail!(
            "{} already exists and is not empty.\n\
             Remove it first or use --ai-root to choose a different path.",
            ai_root.display()
        );
    }

    std::fs::create_dir_all(ai_root)?;
    std::fs::create_dir_all(ai_root.join("tenants"))?;

    let mcp_path = ai_root.join("mcp.json");
    if !mcp_path.exists() {
        std::fs::write(&mcp_path, "{\n  \"mcpServers\": {}\n}\n")?;
    }

    let toml_path = ai_root.join("orbit.toml");
    if !toml_path.exists() {
        std::fs::write(
            &toml_path,
            "# orbit workspace configuration\n# See: https://github.com/befraeloircorona/orbit\n",
        )?;
    }

    println!("Scaffold created at {}", ai_root.display());
    println!();
    println!("  tenants/     — place tenant configs here");
    println!("  mcp.json     — global MCP servers");
    println!("  orbit.toml   — workspace configuration");
    println!();
    println!("  Run `orbit launch` to start a session.");

    Ok(())
}

fn is_empty_dir(path: &std::path::Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
}
