use anyhow::{Result, bail};
use clap::{Args, CommandFactory, Subcommand};
use clap_complete::{Shell, generate};
use std::{io, path::PathBuf};

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    #[command(subcommand)]
    pub command: CompletionsCommand,
}

#[derive(Debug, Subcommand)]
pub enum CompletionsCommand {
    /// Print completion script to stdout
    Print {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Install completions to the correct location for the given shell
    Install {
        /// Shell to install for (auto-detected from $SHELL if omitted)
        #[arg(long, value_enum)]
        shell: Option<Shell>,
        /// Print what would be done without writing any files
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(args: CompletionsArgs) -> Result<()> {
    match args.command {
        CompletionsCommand::Print { shell } => {
            let mut cmd = crate::Cli::command();
            generate(shell, &mut cmd, "orbit", &mut io::stdout());
            Ok(())
        }
        CompletionsCommand::Install { shell, dry_run } => cmd_install(shell, dry_run),
    }
}

fn cmd_install(shell_override: Option<Shell>, dry_run: bool) -> Result<()> {
    let shell = match shell_override {
        Some(s) => s,
        None => detect_shell()?,
    };

    let (dest, source_hint) = install_path(shell)?;
    let content = generate_to_string(shell);

    let label = format!("{:?}", shell).to_lowercase();
    println!("Installing {} completions → {}", label, dest.display());

    if dry_run {
        println!("(dry-run) would write {} bytes", content.len());
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, content)?;
        println!("  Written.");
    }

    if let Some(hint) = source_hint {
        println!();
        println!("  Add this to your shell config (~/.{}rc or similar):", label);
        println!("  {hint}");
    }

    Ok(())
}

fn generate_to_string(shell: Shell) -> String {
    let mut cmd = crate::Cli::command();
    let mut buf = Vec::new();
    generate(shell, &mut cmd, "orbit", &mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn detect_shell() -> Result<Shell> {
    let shell_bin = std::env::var("SHELL").unwrap_or_default();
    let name = shell_bin.split('/').last().unwrap_or("");
    match name {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        "elvish" => Ok(Shell::Elvish),
        _ => bail!(
            "cannot auto-detect shell from $SHELL={shell_bin:?}\n\
             Specify explicitly: orbit completions install --shell <bash|zsh|fish|elvish|powershell>"
        ),
    }
}

fn install_path(shell: Shell) -> Result<(PathBuf, Option<String>)> {
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    match shell {
        Shell::Bash => {
            let dest = home.join(".local/share/bash-completion/completions/orbit");
            let hint = Some(
                "source ~/.local/share/bash-completion/completions/orbit".to_string(),
            );
            Ok((dest, hint))
        }
        Shell::Zsh => {
            let dest = home.join(".local/share/zsh/site-functions/_orbit");
            let hint = Some(format!(
                "fpath=(~/.local/share/zsh/site-functions $fpath)\nautoload -U compinit && compinit"
            ));
            Ok((dest, hint))
        }
        Shell::Fish => {
            let dest = home.join(".config/fish/completions/orbit.fish");
            Ok((dest, None)) // fish auto-loads from this dir
        }
        Shell::Elvish => {
            let dest = home.join(".config/elvish/lib/orbit.elv");
            let hint = Some("use orbit".to_string());
            Ok((dest, hint))
        }
        Shell::PowerShell => {
            let dest = home.join("Documents/PowerShell/orbit.ps1");
            let hint = Some(". ~/Documents/PowerShell/orbit.ps1".to_string());
            Ok((dest, hint))
        }
        _ => bail!("unsupported shell for automatic install"),
    }
}
