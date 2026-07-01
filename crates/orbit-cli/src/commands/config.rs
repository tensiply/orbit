use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use orbit_core::user_config::UserConfig;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print a single config value
    Get {
        /// Config key (e.g. engine.default)
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key (e.g. engine.default)
        key: String,
        /// New value
        value: String,
    },
    /// Print all config values
    List,
    /// Open config file in $EDITOR
    Edit,
}

const VALID_KEYS: &[&str] = &[
    "workspace.ai_root",
    "engine.default",
    "engine.default_tenant",
    "engine.default_workspace",
    "install.dir",
];

pub fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Get { key } => cmd_get(&key),
        ConfigCommand::Set { key, value } => cmd_set(&key, &value),
        ConfigCommand::List => cmd_list(),
        ConfigCommand::Edit => cmd_edit(),
    }
}

fn cmd_get(key: &str) -> Result<()> {
    let cfg = UserConfig::load();
    println!("{}", get_value(&cfg, key)?);
    Ok(())
}

fn cmd_set(key: &str, value: &str) -> Result<()> {
    let mut cfg = UserConfig::load();
    set_value(&mut cfg, key, value)?;
    cfg.save()?;
    println!("  {} = {}", key, value);
    Ok(())
}

fn cmd_list() -> Result<()> {
    let cfg = UserConfig::load();
    println!("# {}", UserConfig::path().display());
    println!();
    for key in VALID_KEYS {
        println!("  {} = {}", key, get_value(&cfg, key).unwrap_or_default());
    }
    Ok(())
}

fn cmd_edit() -> Result<()> {
    let path = UserConfig::path();
    if !path.exists() {
        UserConfig::default().save()?;
    }
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        bail!("editor exited with non-zero status");
    }
    Ok(())
}

fn get_value(cfg: &UserConfig, key: &str) -> Result<String> {
    Ok(match key {
        "workspace.ai_root" => cfg.workspace.ai_root.to_string_lossy().into_owned(),
        "engine.default" => cfg.engine.default.clone(),
        "engine.default_tenant" => cfg.engine.default_tenant.clone(),
        "engine.default_workspace" => cfg.engine.default_workspace.clone(),
        "install.dir" => cfg.install.dir.to_string_lossy().into_owned(),
        other => bail!(
            "unknown key: {other}\n\n  Valid keys:\n{}",
            VALID_KEYS.iter().map(|k| format!("    {k}")).collect::<Vec<_>>().join("\n")
        ),
    })
}

fn set_value(cfg: &mut UserConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "workspace.ai_root" => cfg.workspace.ai_root = PathBuf::from(value),
        "engine.default" => {
            let valid: Vec<String> = orbit_core::catalog::engines()
                .into_iter()
                .map(|e| e.name)
                .collect();
            if !valid.contains(&value.to_string()) {
                bail!(
                    "invalid engine: {value}  (valid: {})",
                    valid.join(", ")
                );
            }
            cfg.engine.default = value.to_string();
        }
        "engine.default_tenant" => cfg.engine.default_tenant = value.to_string(),
        "engine.default_workspace" => {
            if value.is_empty() {
                cfg.engine.default_workspace = String::new();
            } else {
                let home = directories::BaseDirs::new()
                    .map(|b| b.home_dir().to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("/"));
                let ws_path = home.join(value);
                if !ws_path.is_dir() {
                    bail!(
                        "workspace not found: ~/{value}\n\n  Expected a directory at {}",
                        ws_path.display()
                    );
                }
                cfg.engine.default_workspace = value.to_string();
            }
        }
        "install.dir" => cfg.install.dir = PathBuf::from(value),
        other => bail!(
            "unknown key: {other}\n\n  Valid keys:\n{}",
            VALID_KEYS.iter().map(|k| format!("    {k}")).collect::<Vec<_>>().join("\n")
        ),
    }
    Ok(())
}
