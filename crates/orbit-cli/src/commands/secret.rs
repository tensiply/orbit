use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use orbit_core::secrets;

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct SecretArgs {
    #[command(subcommand)]
    pub command: SecretCommand,
}

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    /// Store a secret in the OS keychain
    Set {
        /// Key name (referenced in orbit.json as `keychain://KEY`)
        key: String,
        /// Secret value (omit to read from stdin)
        value: Option<String>,
    },
    /// Retrieve a secret from the OS keychain
    Get {
        /// Key name
        key: String,
    },
    /// Delete a secret from the OS keychain
    Delete {
        /// Key name
        key: String,
    },
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run(args: SecretArgs) -> Result<()> {
    match args.command {
        SecretCommand::Set { key, value } => cmd_set(&key, value),
        SecretCommand::Get { key } => cmd_get(&key),
        SecretCommand::Delete { key } => cmd_delete(&key),
    }
}

// ── handlers ──────────────────────────────────────────────────────────────────

fn cmd_set(key: &str, value: Option<String>) -> Result<()> {
    let secret = match value {
        Some(v) => v,
        None => read_secret_from_stdin(key)?,
    };
    secrets::keychain_set(key, &secret)?;
    println!("secret '{key}' stored in keychain");
    println!("reference it in orbit.json as: \"keychain://{key}\"");
    Ok(())
}

fn cmd_get(key: &str) -> Result<()> {
    let secret = secrets::keychain_get(key)?;
    println!("{secret}");
    Ok(())
}

fn cmd_delete(key: &str) -> Result<()> {
    secrets::keychain_delete(key)?;
    println!("secret '{key}' deleted from keychain");
    Ok(())
}

fn read_secret_from_stdin(key: &str) -> Result<String> {
    use std::io::{self, BufRead, IsTerminal, Write};
    let stdin = io::stdin();
    if stdin.is_terminal() {
        eprint!("enter value for '{key}': ");
        io::stderr().flush()?;
        // Read without echoing if possible (best-effort — no rpassword dep)
        let line = stdin.lock().lines().next();
        match line {
            Some(Ok(v)) if !v.is_empty() => Ok(v),
            _ => bail!("no value provided"),
        }
    } else {
        let line = stdin.lock().lines().next();
        match line {
            Some(Ok(v)) if !v.is_empty() => Ok(v),
            _ => bail!("no value provided via stdin"),
        }
    }
}
