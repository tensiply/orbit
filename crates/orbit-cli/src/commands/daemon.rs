use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_client::ipc;
use serde_json;
use std::time::Duration;

#[derive(Debug, Args)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonCommand,
}

#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    /// Start the orbit daemon in the background
    Start,
    /// Stop the running orbit daemon
    Stop,
    /// Show daemon status (uptime, sessions)
    Status,
    /// Show daemon health diagnostics (plans, costs, archival)
    Health {
        /// Output raw JSON instead of formatted table
        #[arg(long)]
        json: bool,
    },
    /// [INTERNAL] Run the daemon server in the foreground (used by `start`)
    #[command(hide = true)]
    Serve,
}

pub async fn run(args: DaemonArgs) -> Result<()> {
    match args.command {
        DaemonCommand::Start => start().await,
        DaemonCommand::Stop => stop().await,
        DaemonCommand::Status => status().await,
        DaemonCommand::Health { json } => health(json).await,
        DaemonCommand::Serve => serve().await,
    }
}

// ── start ─────────────────────────────────────────────────────────────────────

async fn start() -> Result<()> {
    if ipc::is_available() {
        // Try a real connection to verify it's alive, not just a stale socket
        if let Ok(info) = ipc::status().await {
            println!(
                "Daemon is already running (pid {}, uptime {}s).",
                info.pid, info.uptime_secs
            );
            return Ok(());
        }
        // Stale socket — clean it up and continue
    }

    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .args(["daemon", "serve"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()?;

    // Give the daemon a moment to bind the socket
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if ipc::is_available() {
            break;
        }
    }

    if ipc::is_available() {
        println!("Daemon started.");
    } else {
        println!("Daemon launched but socket not yet ready — it may still be starting.");
    }

    Ok(())
}

// ── stop ──────────────────────────────────────────────────────────────────────

async fn stop() -> Result<()> {
    if !ipc::is_available() {
        println!("Daemon is not running.");
        return Ok(());
    }

    ipc::shutdown().await?;
    println!("Daemon stopped.");
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

async fn status() -> Result<()> {
    use orbit_core::channel::Channel;
    let ch = Channel::current();
    println!("Channel:  {}", ch.as_str());
    println!(
        "  Home:   {}",
        orbit_core::data_paths::orbit_home().display()
    );
    println!("  Socket: {}", orbit_core::ipc::socket_path().display());
    println!("  Debug:  127.0.0.1:{} (desktop MCP)", ch.debug_port_base());

    if !ipc::is_available() {
        println!("Daemon: not running");
        return Ok(());
    }

    match ipc::status().await {
        Ok(info) => {
            let uptime = format_uptime(info.uptime_secs);
            println!("Daemon: running");
            println!("  PID:      {}", info.pid);
            println!("  Uptime:   {uptime}");
            println!("  Sessions: {} active", info.session_count);
            // The daemon reports the channel it is bound to — flag a mismatch so
            // a cross-channel confusion surfaces instead of passing silently.
            if !info.channel.is_empty() && info.channel != ch.as_str() {
                println!(
                    "  ⚠ channel mismatch: daemon serves '{}' ({}) but this binary is '{}'",
                    info.channel,
                    info.home,
                    ch.as_str()
                );
            }
        }
        Err(e) => {
            println!("Daemon: socket exists but not responding ({e})");
        }
    }

    Ok(())
}

// ── health ────────────────────────────────────────────────────────────────────

async fn health(json: bool) -> Result<()> {
    if !ipc::is_available() {
        println!("Daemon: not running");
        return Ok(());
    }

    match ipc::health().await {
        Ok(h) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "uptime_secs": h.uptime_secs,
                        "pid": h.pid,
                        "running_plans": h.running_plans,
                        "completed_today": h.completed_today,
                        "failed_today": h.failed_today,
                        "plan_files": h.plan_files,
                        "archived_plans": h.archived_plans,
                        "memory_records": h.memory_records,
                        "auto_prune_enabled": h.auto_prune_enabled,
                        "auto_prune_days": h.auto_prune_days,
                    })
                );
            } else {
                println!("Daemon health");
                println!("  PID:              {}", h.pid);
                println!("  Uptime:           {}", format_uptime(h.uptime_secs));
                println!();
                println!("Plans");
                println!("  Running:          {}", h.running_plans);
                println!("  Completed today:  {}", h.completed_today);
                println!("  Failed today:     {}", h.failed_today);
                println!("  Plan files:       {}", h.plan_files);
                println!("  Archived:         {}", h.archived_plans);
                println!();
                println!("Memory");
                println!("  History records:  {}", h.memory_records);
                println!();
                println!("Retention");
                if h.auto_prune_enabled {
                    println!("  Auto-prune:       enabled ({} days)", h.auto_prune_days);
                } else {
                    println!("  Auto-prune:       disabled");
                    println!(
                        "  Enable with:      orbit config set plan_retention.auto_prune_enabled true"
                    );
                }
            }
        }
        Err(e) => {
            println!("Daemon: socket exists but not responding ({e})");
        }
    }

    Ok(())
}

// ── serve (hidden, runs the actual daemon) ────────────────────────────────────

async fn serve() -> Result<()> {
    let name = orbit_core::channel::Channel::current().daemon_process_name();
    #[cfg(target_os = "linux")]
    let _ = std::fs::write("/proc/self/comm", &name);
    tracing::info!("{} starting", name);
    orbit_daemon::server::run().await
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
