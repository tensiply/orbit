use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkRole {
    Observer,
    Contributor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitClaims {
    pub role: NetworkRole,
    pub exp: u64,
    pub iat: u64,
    pub scope: String,
    pub instance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPeerInfo {
    pub addr: String,
    pub role: NetworkRole,
    pub connected_at: u64,
    pub requests: u64,
}
