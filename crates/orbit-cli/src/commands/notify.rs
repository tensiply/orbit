use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::{notify, user_config::UserConfig};

#[derive(Debug, Args)]
pub struct NotifyArgs {
    #[command(subcommand)]
    pub command: Option<NotifyCommand>,
}

#[derive(Debug, Subcommand)]
pub enum NotifyCommand {
    /// Send a test notification to verify the backend works
    Test,
    /// Enable desktop notifications for plan completions and failures
    Enable,
    /// Disable all desktop notifications
    Disable,
    /// Show notification settings and backend status
    Status,
}

pub fn run(args: NotifyArgs) -> Result<()> {
    match args.command.unwrap_or(NotifyCommand::Status) {
        NotifyCommand::Test => cmd_test(),
        NotifyCommand::Enable => cmd_set_enabled(true),
        NotifyCommand::Disable => cmd_set_enabled(false),
        NotifyCommand::Status => cmd_status(),
    }
}

// ── commands ──────────────────────────────────────────────────────────────────

fn cmd_test() -> Result<()> {
    if !notify::backend_available() {
        println!("  \x1b[33m!\x1b[0m  Notification backend not available.");
        println!();
        println!("     Linux : sudo apt install libnotify-bin");
        println!("     macOS : osascript is built-in (no install needed)");
        return Ok(());
    }
    notify::send_notification("orbit · Test", "Notifications are working!");
    println!("  \x1b[32m✓\x1b[0m  Test notification sent.");
    Ok(())
}

fn cmd_set_enabled(enabled: bool) -> Result<()> {
    let mut cfg = UserConfig::load();
    cfg.notifications.enabled = enabled;
    cfg.save()?;
    let (icon, label) = if enabled {
        ("\x1b[32m✓\x1b[0m", "enabled")
    } else {
        ("\x1b[33m○\x1b[0m", "disabled")
    };
    println!("  {icon}  Desktop notifications {label}.");
    if enabled && !notify::backend_available() {
        println!();
        println!(
            "  \x1b[33m!\x1b[0m  Backend not found — install libnotify-bin (Linux) or use macOS."
        );
        println!("     Run `orbit notify test` after installing.");
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = UserConfig::load();
    let nc = &cfg.notifications;
    let available = notify::backend_available();

    let enabled_label = if nc.enabled {
        "\x1b[32menabled\x1b[0m"
    } else {
        "\x1b[33mdisabled\x1b[0m"
    };
    let backend_label = if available {
        "\x1b[32mavailable\x1b[0m"
    } else {
        "\x1b[31mnot found\x1b[0m"
    };

    println!();
    println!("  \x1b[1mnotifications\x1b[0m");
    println!();
    println!("    enabled           {enabled_label}");
    println!("    backend           {backend_label}");
    println!("    on plan complete  {}", yn(nc.on_plan_complete));
    println!("    on plan failed    {}", yn(nc.on_plan_failed));
    println!();

    if nc.enabled {
        if !available {
            println!("  \x1b[33m!\x1b[0m  Notifications enabled but backend not found.");
            println!("     Linux : sudo apt install libnotify-bin");
            println!("     macOS : built-in (osascript)");
        } else {
            println!("  Run `orbit notify test` to verify.");
        }
    } else {
        println!("  Run `orbit notify enable` to turn on desktop notifications.");
    }

    println!();
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn yn(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}
