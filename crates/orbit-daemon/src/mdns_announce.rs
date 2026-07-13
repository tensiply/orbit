use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;

pub struct MdnsAnnouncer {
    daemon: ServiceDaemon,
    fullname: String,
}

pub struct AnnounceConfig<'a> {
    pub instance_name: &'a str,
    pub port: u16,
    pub observer_token: &'a str,
    pub hostname: &'a str,
}

const SERVICE_TYPE: &str = "_orbit._tcp.local.";

impl MdnsAnnouncer {
    pub fn start(cfg: AnnounceConfig<'_>) -> Result<Self> {
        let daemon = ServiceDaemon::new()?;

        let mut properties = HashMap::new();
        properties.insert("v".to_string(), "1".to_string());
        properties.insert("port".to_string(), cfg.port.to_string());
        let obs_truncated: String = cfg.observer_token.chars().take(200).collect();
        properties.insert("obs".to_string(), obs_truncated);

        let host_with_dot = if cfg.hostname.ends_with('.') {
            cfg.hostname.to_string()
        } else {
            format!("{}.", cfg.hostname)
        };

        let info = ServiceInfo::new(
            SERVICE_TYPE,
            cfg.instance_name,
            &host_with_dot,
            (),
            cfg.port,
            properties,
        )?;

        let fullname = info.get_fullname().to_string();
        daemon.register(info)?;

        Ok(Self { daemon, fullname })
    }

    pub fn stop(self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}
