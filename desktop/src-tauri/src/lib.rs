mod architecture;
mod daemon;
mod documents;
mod plugins;
mod pty;
mod scopes;
mod sessions;
mod workspaces;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "orbit_desktop=debug".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            // Sessions
            sessions::session_list,
            sessions::session_kill,
            sessions::session_clean,
            sessions::session_launch,
            sessions::get_session_title,
            // PTY
            pty::pty_open,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_close,
            // Daemon
            daemon::daemon_status,
            daemon::daemon_ensure_running,
            // Plugins
            plugins::plugin_list,
            plugins::plugin_enable,
            plugins::plugin_disable,
            // Documents
            documents::document_list,
            documents::document_read_b64,
            documents::document_delete,
            documents::document_archive,
            documents::document_reveal,
            // Workspaces
            workspaces::workspace_list,
            // Scopes
            scopes::scope_tree,
            // Architecture
            architecture::architecture_load,
            architecture::architecture_save_entity,
            architecture::architecture_delete_entity,
            architecture::architecture_load_layout,
            architecture::architecture_save_layout,
            architecture::architecture_load_routes,
            architecture::architecture_save_routes,
        ])
        .setup(|app| {
            // Auto-start daemon on launch
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = daemon::ensure_running(&app_handle).await {
                    tracing::warn!("daemon auto-start: {e}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running orbit desktop");
}
