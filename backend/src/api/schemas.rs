use serde::{Deserialize, Serialize};

// ── Topology requests ──

#[derive(Debug, Deserialize)]
pub struct CreateAsRequest {
    pub asn: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRouterRequest {
    pub name: String,
    pub router_id_ip: String,
    pub position_x: f64,
    pub position_y: f64,
}

#[derive(Debug, Deserialize)]
pub struct AddInterfaceRequest {
    pub name: String,
    pub ip_address: String,
    pub bandwidth: u64,
    pub cost: u32,
}

#[derive(Debug, Deserialize)]
pub struct CreateLinkRequest {
    pub interface_a_id: String,
    pub interface_b_id: String,
    pub bandwidth: u64,
    pub delay_ms: f64,
}

#[derive(Debug, Deserialize)]
pub struct LinkStateRequest {
    pub is_up: bool,
}

// ── BGP requests ──

#[derive(Debug, Deserialize)]
pub struct EnableBgpRequest {
    pub local_asn: u32,
}

#[derive(Debug, Deserialize)]
pub struct AddBgpNeighborRequest {
    pub neighbor_ip: String,
    pub remote_asn: u32,
    #[serde(default)]
    pub import_policy: Option<String>,
    #[serde(default)]
    pub export_policy: Option<String>,
}

// ── Traffic requests ──

#[derive(Debug, Deserialize)]
pub struct CreateTrafficGeneratorRequest {
    pub source_router_id: String,
    pub dest_prefix: String,
    pub rate_bps: u64,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTrafficGeneratorRequest {
    pub rate_bps: Option<u64>,
    pub is_active: Option<bool>,
    pub dest_prefix: Option<String>,
}

// ── Simulation requests ──

#[derive(Debug, Deserialize)]
pub struct TickRateRequest {
    pub tick_rate_ms: u64,
}

// ── Policy requests ──

#[derive(Debug, Deserialize)]
pub struct PolicyRequest {
    pub policy_text: String,
}

// ── Generic responses ──

#[derive(Debug, Serialize)]
pub struct IdResponse {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}
