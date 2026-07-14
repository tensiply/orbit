use anyhow::Result;
use clap::Args;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::time::{Duration, Instant};

#[derive(Debug, Args)]
pub struct DiscoverArgs {
    /// How many seconds to scan
    #[arg(long, default_value = "5")]
    pub timeout: u64,
}

pub struct DiscoveredInstance {
    pub instance_name: String,
    pub hostname: String,
    pub port: u16,
    pub observer_token: Option<String>,
}

pub fn run(args: DiscoverArgs) -> Result<()> {
    let daemon = ServiceDaemon::new()?;
    let receiver = daemon.browse("_orbit._tcp.local.")?;

    let deadline = Instant::now() + Duration::from_secs(args.timeout);
    let mut instances = Vec::new();

    println!("Scanning for orbit instances ({} seconds)...", args.timeout);

    while Instant::now() < deadline {
        let remaining = deadline - Instant::now();
        if let Ok(ServiceEvent::ServiceResolved(info)) =
            receiver.recv_timeout(remaining.min(Duration::from_millis(200)))
        {
            let props = info.get_properties();
            let obs = props.get("obs").map(|v| v.val_str().to_string());
            instances.push(DiscoveredInstance {
                instance_name: info.get_fullname().to_string(),
                hostname: info.get_hostname().to_string(),
                port: info.get_port(),
                observer_token: obs,
            });
            println!(
                "  Found: {} at {}:{}",
                info.get_fullname(),
                info.get_hostname(),
                info.get_port()
            );
        }
    }

    let _ = daemon.shutdown();

    if instances.is_empty() {
        println!("No orbit instances found on the local network.");
        return Ok(());
    }

    println!("\n{} instance(s) found:", instances.len());
    println!("{:<30} {:<20} {:<6} TOKEN", "INSTANCE", "HOST", "PORT");
    println!("{}", "-".repeat(80));
    for inst in &instances {
        let token_hint = inst
            .observer_token
            .as_deref()
            .map(|t| format!("obs:{}", &t[..t.len().min(20)]))
            .unwrap_or_else(|| "—".to_string());
        println!(
            "{:<30} {:<20} {:<6} {}",
            inst.instance_name, inst.hostname, inst.port, token_hint
        );
    }

    Ok(())
}
