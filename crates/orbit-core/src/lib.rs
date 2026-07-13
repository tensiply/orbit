pub mod audit;
pub mod catalog;
pub mod config;
pub mod context;
pub mod data_paths;
pub mod engine;
pub mod error;
pub mod eval;
pub mod hooks;
pub mod ipc;
pub mod jira;
pub mod memory;
pub mod notify;
pub mod plan;
pub mod plugin;
pub mod resolver;
pub mod schedule;
pub mod secrets;
pub mod session;
pub mod template;
pub mod user_config;
pub mod workspace_config;
pub mod workspace_registry;

/// Serialises tests that mutate XDG_DATA_HOME so they don't race in parallel.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
