use anyhow::Result;
use orbit_core::{
    ipc::{PlanStreamEvent, Request, Response},
    net::{NetworkPeerInfo, NetworkRole},
    plan::PlanStatus,
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::broadcast,
    task::JoinHandle,
};
use tracing::{debug, info, warn};

use crate::{
    auth::verify_token,
    server::{ConnectionRole, ServerState},
};

// ── peers registry ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PeersRegistry {
    inner: Arc<Mutex<Vec<PeerEntry>>>,
}

struct PeerEntry {
    addr: SocketAddr,
    role: NetworkRole,
    connected_at: Instant,
    requests: u64,
}

impl Default for PeersRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl PeersRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, addr: SocketAddr, role: NetworkRole) {
        self.inner.lock().unwrap().push(PeerEntry {
            addr,
            role,
            connected_at: Instant::now(),
            requests: 0,
        });
    }

    pub fn remove(&self, addr: &SocketAddr) {
        self.inner.lock().unwrap().retain(|p| &p.addr != addr);
    }

    pub fn inc_requests(&self, addr: &SocketAddr) {
        let mut peers = self.inner.lock().unwrap();
        if let Some(p) = peers.iter_mut().find(|p| &p.addr == addr) {
            p.requests += 1;
        }
    }

    pub fn list(&self) -> Vec<NetworkPeerInfo> {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.inner
            .lock()
            .unwrap()
            .iter()
            .map(|p| NetworkPeerInfo {
                addr: p.addr.to_string(),
                role: p.role,
                connected_at: now_secs.saturating_sub(p.connected_at.elapsed().as_secs()),
                requests: p.requests,
            })
            .collect()
    }
}

// ── TCP bridge handle ────────────────────────────────────────────────────────

pub struct TcpBridgeHandle {
    _accept_task: JoinHandle<()>,
    pub peers: PeersRegistry,
    pub port: u16,
}

pub struct TcpBridgeConfig {
    pub port: u16,
    pub max_role: NetworkRole,
    pub signing_key: [u8; 32],
}

pub(crate) async fn start(
    config: TcpBridgeConfig,
    state: Arc<ServerState>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<TcpBridgeHandle> {
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;
    let actual_port = listener.local_addr()?.port();
    info!("orbit TCP bridge listening on {}", listener.local_addr()?);

    let peers = PeersRegistry::new();
    let peers_clone = peers.clone();
    let key = config.signing_key;
    let max_role = config.max_role;

    let accept_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                accept = listener.accept() => {
                    match accept {
                        Ok((stream, addr)) => {
                            let state = state.clone();
                            let peers = peers_clone.clone();
                            tokio::spawn(handle_tcp_connection(stream, addr, state, key, max_role, peers));
                        }
                        Err(e) => { warn!("TCP accept error: {e}"); break; }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("TCP bridge shutting down");
                    break;
                }
            }
        }
    });

    Ok(TcpBridgeHandle {
        _accept_task: accept_task,
        peers,
        port: actual_port,
    })
}

// ── TCP connection handler ────────────────────────────────────────────────────

async fn handle_tcp_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ServerState>,
    signing_key: [u8; 32],
    max_role: NetworkRole,
    peers: PeersRegistry,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // First line must be the auth token.
    let auth_token = match lines.next_line().await {
        Ok(Some(line)) => line,
        _ => {
            let _ = write_response(
                &mut writer,
                &Response::Error {
                    message: "expected auth token as first line".into(),
                },
            )
            .await;
            return;
        }
    };

    let claims = match verify_token(auth_token.trim(), &signing_key) {
        Ok(c) => c,
        Err(e) => {
            let _ = write_response(
                &mut writer,
                &Response::Error {
                    message: format!("auth failed: {e}"),
                },
            )
            .await;
            return;
        }
    };

    // Cap role to server's max_role.
    let effective_role = match (claims.role, max_role) {
        (NetworkRole::Contributor, NetworkRole::Observer) => NetworkRole::Observer,
        (role, _) => role,
    };

    let conn_role = match effective_role {
        NetworkRole::Observer => ConnectionRole::Observer,
        NetworkRole::Contributor => ConnectionRole::Contributor,
    };

    info!("TCP peer {addr} connected as {effective_role:?}");
    peers.add(addr, effective_role);

    while let Ok(Some(line)) = lines.next_line().await {
        debug!("TCP ipc request from {addr}: {line}");
        peers.inc_requests(&addr);

        let req = match serde_json::from_str::<Request>(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = write_response(
                    &mut writer,
                    &Response::Error {
                        message: format!("parse error: {e}"),
                    },
                )
                .await;
                break;
            }
        };

        // StreamPlan: forward events until terminal.
        if let Request::StreamPlan { ref id } = req {
            if !conn_role.allows(&req) {
                let _ = write_response(
                    &mut writer,
                    &Response::Error {
                        message: "operation not permitted".into(),
                    },
                )
                .await;
                break;
            }
            let plan_id = id.clone();
            let mut rx = state.event_tx.subscribe();

            let current_terminal =
                orbit_core::plan::Plan::load(&plan_id)
                    .ok()
                    .and_then(|p| match p.status {
                        PlanStatus::Completed => Some(PlanStreamEvent::PlanCompleted {
                            plan_id: plan_id.clone(),
                        }),
                        PlanStatus::Failed | PlanStatus::Cancelled => {
                            Some(PlanStreamEvent::PlanFailed {
                                plan_id: plan_id.clone(),
                            })
                        }
                        _ => None,
                    });

            if let Some(ev) = current_terminal {
                let _ = write_event(&mut writer, &ev).await;
                break;
            }

            loop {
                match rx.recv().await {
                    Ok(event) if event.plan_id() == plan_id.as_str() => {
                        let terminal = event.is_terminal();
                        if write_event(&mut writer, &event).await.is_err() {
                            break;
                        }
                        if terminal {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            break;
        }

        let response = state.handle(req, conn_role);
        if write_response(&mut writer, &response).await.is_err() {
            break;
        }
    }

    peers.remove(&addr);
    info!("TCP peer {addr} disconnected");
}

async fn write_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    resp: &Response,
) -> Result<()> {
    let mut json = serde_json::to_string(resp)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    Ok(())
}

async fn write_event(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    event: &PlanStreamEvent,
) -> Result<()> {
    let mut json = serde_json::to_string(event)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    Ok(())
}
