use anyhow::Result;
use clap::{Args, CommandFactory};
use clap_mangen::Man;
use std::{fs, io, path::PathBuf};

#[derive(Debug, Args)]
pub struct ManArgs {
    /// Subcommand to generate a man page for (empty = top-level orbit(1))
    #[arg(value_name = "SUBCOMMAND")]
    pub subcommand: Option<String>,
    /// Install all man pages to a directory instead of printing to stdout
    #[arg(long, value_name = "DIR")]
    pub install: Option<PathBuf>,
}

pub fn run(args: ManArgs) -> Result<()> {
    if let Some(dir) = args.install {
        return cmd_install(&dir);
    }
    cmd_print(args.subcommand.as_deref())
}

fn cmd_print(subcommand: Option<&str>) -> Result<()> {
    let root = crate::Cli::command();

    if let Some(sub) = subcommand {
        let sub_cmd = root.find_subcommand(sub).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "unknown subcommand: {sub}\n\nRun `orbit --help` to see available subcommands."
            )
        })?;
        Man::new(sub_cmd).render(&mut io::stdout())?;
    } else {
        Man::new(root).render(&mut io::stdout())?;
    }

    Ok(())
}

fn cmd_install(dir: &PathBuf) -> Result<()> {
    fs::create_dir_all(dir)?;
    let root = crate::Cli::command();

    // Top-level page
    let page_path = dir.join("orbit.1");
    let mut f = fs::File::create(&page_path)?;
    Man::new(root.clone()).render(&mut f)?;
    println!("  orbit.1 → {}", page_path.display());

    // One page per subcommand
    for sub in root.get_subcommands() {
        let name = sub.get_name();
        let filename = format!("orbit-{}.1", name);
        let path = dir.join(&filename);
        let mut f = fs::File::create(&path)?;
        Man::new(sub.clone()).render(&mut f)?;
        println!("  {filename} → {}", path.display());
    }

    println!();
    println!("To use these pages:");
    println!("  export MANPATH=\"{}:$MANPATH\"", dir.display());
    println!("  man orbit");
    Ok(())
}
