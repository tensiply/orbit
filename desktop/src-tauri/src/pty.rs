use anyhow::Result;
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PtyDataEvent {
    pub tab_id: String,
    pub data: String,
}

struct PtyHandle {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    _child: Box<dyn Child + Send + Sync>,
}

static PTY_MAP: OnceLock<Arc<Mutex<HashMap<String, PtyHandle>>>> = OnceLock::new();

fn pty_map() -> &'static Arc<Mutex<HashMap<String, PtyHandle>>> {
    PTY_MAP.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

/// Open a PTY and return the tab_id.
/// If `tmux_session` is Some, attaches to that tmux session.
/// Otherwise opens the user's default shell.
#[tauri::command]
pub async fn pty_open(
    app: AppHandle,
    tmux_session: Option<String>,
) -> Result<String, String> {
    let tab_id = Uuid::new_v4().to_string();
    let tid = tab_id.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| anyhow::anyhow!("openpty: {e}"))?;

        let cmd = if let Some(ref name) = tmux_session {
            let mut c = CommandBuilder::new("tmux");
            c.args(["attach-session", "-t", name]);
            c
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
            CommandBuilder::new(shell)
        };

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| anyhow::anyhow!("spawn: {e}"))?;

        // Clone reader before moving master into handle
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| anyhow::anyhow!("clone reader: {e}"))?;

        // take_writer must be called before moving master — can only be called once
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| anyhow::anyhow!("take writer: {e}"))?;

        let tid_reader = tid.clone();
        let app_reader = app.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buf[..n]).into_owned();
                        let _ = app_reader.emit(
                            "pty-data",
                            PtyDataEvent {
                                tab_id: tid_reader.clone(),
                                data,
                            },
                        );
                    }
                }
            }
        });

        pty_map().lock().unwrap().insert(
            tid,
            PtyHandle {
                writer,
                master: pair.master,
                _child: child,
            },
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    Ok(tab_id)
}

#[tauri::command]
pub async fn pty_write(tab_id: String, data: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut map = pty_map().lock().unwrap();
        if let Some(handle) = map.get_mut(&tab_id) {
            handle
                .writer
                .write_all(data.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn pty_resize(tab_id: String, cols: u16, rows: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let map = pty_map().lock().unwrap();
        if let Some(handle) = map.get(&tab_id) {
            handle
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn pty_close(tab_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        pty_map().lock().unwrap().remove(&tab_id);
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}
