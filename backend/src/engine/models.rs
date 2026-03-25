use ipnetwork::Ipv4Network;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;

// ── Core topology ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub id: String,
    pub router_id: String,
    pub ip_address: Ipv4Network,
    pub link_id: Option<String>,
    pub bandwidth: u64,
    pub cost: u32,
    pub is_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: String,
    pub interface_a_id: String,
    pub interface_b_id: String,
    pub bandwidth: u64,
    pub current_load: f64,
    pub delay_ms: f64,
    pub is_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Router {
    pub id: String,
    pub as_id: String,
    pub name: String,
    pub router_id_ip: Ipv4Addr,
    pub interfaces: HashMap<String, Interface>,
    pub ospf_process: Option<OspfProcess>,
    pub bgp_process: Option<BgpProcess>,
    pub rib: HashMap<String, RibEntry>,
    pub fib: HashMap<String, FibEntry>,
    pub traffic_generators: Vec<TrafficGenerator>,
    pub position: (f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousSystem {
    pub id: String,
    pub asn: u32,
    pub name: String,
    pub routers: HashMap<String, Router>,
    pub ospf_areas: HashMap<u32, HashSet<String>>,
}

// ── RIB / FIB ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RibEntry {
    pub prefix: String,
    pub next_hop: Ipv4Addr,
    pub protocol: RouteProtocol,
    pub metric: u32,
    pub admin_distance: u8,
    pub bgp_attributes: Option<BgpAttributes>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RouteProtocol {
    Connected,
    Ospf,
    BgpExternal,
    BgpInternal,
}

impl RouteProtocol {
    pub fn admin_distance(&self) -> u8 {
        match self {
            RouteProtocol::Connected => 0,
            RouteProtocol::Ospf => 110,
            RouteProtocol::BgpExternal => 20,
            RouteProtocol::BgpInternal => 200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FibEntry {
    pub prefix: String,
    pub next_hop_ip: Ipv4Addr,
    pub out_interface_id: String,
}

// ── OSPF ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OspfProcess {
    pub router_id: Ipv4Addr,
    pub areas: HashMap<u32, OspfArea>,
    pub lsdb: HashMap<String, Lsa>,
    pub neighbors: HashMap<String, OspfNeighbor>,
    pub spf_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OspfArea {
    pub area_id: u32,
    pub interface_ids: Vec<String>,
    pub is_backbone: bool,
    pub area_type: OspfAreaType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OspfAreaType {
    Normal,
    Stub,
    Nssa,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OspfNeighbor {
    pub neighbor_id: Ipv4Addr,
    pub state: OspfNeighborState,
    pub interface_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OspfNeighborState {
    Down,
    Init,
    TwoWay,
    ExStart,
    Exchange,
    Loading,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lsa {
    pub lsa_type: LsaType,
    pub advertising_router: Ipv4Addr,
    pub link_state_id: String,
    pub sequence_number: u32,
    pub age: u16,
    pub body: LsaBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LsaType {
    RouterLsa,
    NetworkLsa,
    SummaryLsa,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LsaBody {
    Router {
        links: Vec<RouterLsaLink>,
    },
    Network {
        attached_routers: Vec<Ipv4Addr>,
        network_mask: Ipv4Network,
    },
    Summary {
        prefix: Ipv4Network,
        metric: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterLsaLink {
    pub link_id: String,
    pub link_data: Ipv4Addr,
    pub link_type: RouterLsaLinkType,
    pub metric: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RouterLsaLinkType {
    PointToPoint,
    TransitNetwork,
    StubNetwork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpfResult {
    pub destination: Ipv4Network,
    pub cost: u32,
    pub next_hop: Ipv4Addr,
    pub next_hop_interface_id: String,
}

// ── BGP ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgpProcess {
    pub local_asn: u32,
    pub router_id: Ipv4Addr,
    pub neighbors: HashMap<String, BgpNeighbor>,
    pub loc_rib: HashMap<String, Vec<BgpRoute>>,
    pub best_routes: HashMap<String, BgpRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgpNeighbor {
    pub neighbor_ip: Ipv4Addr,
    pub remote_asn: u32,
    pub state: BgpSessionState,
    pub is_ebgp: bool,
    pub adj_rib_in: HashMap<String, BgpRoute>,
    pub adj_rib_out: HashMap<String, BgpRoute>,
    pub import_policy: Option<String>,
    pub export_policy: Option<String>,
    pub uptime_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BgpSessionState {
    Idle,
    Connect,
    OpenSent,
    OpenConfirm,
    Established,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgpRoute {
    pub prefix: String,
    pub attributes: BgpAttributes,
    pub received_from: Option<Ipv4Addr>,
    pub is_best: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgpAttributes {
    pub as_path: Vec<u32>,
    pub local_pref: u32,
    pub med: u32,
    pub next_hop: Ipv4Addr,
    pub origin: BgpOrigin,
    pub communities: Vec<String>,
    pub weight: u32,
}

impl Default for BgpAttributes {
    fn default() -> Self {
        Self {
            as_path: Vec::new(),
            local_pref: 100,
            med: 0,
            next_hop: Ipv4Addr::UNSPECIFIED,
            origin: BgpOrigin::Igp,
            communities: Vec::new(),
            weight: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BgpOrigin {
    Igp,
    Egp,
    Incomplete,
}

// ── Traffic ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficGenerator {
    pub id: String,
    pub source_router_id: String,
    pub dest_prefix: String,
    pub rate_bps: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficFlow {
    pub generator_id: String,
    pub path: Vec<String>,
    pub rate_bps: u64,
    pub is_blackholed: bool,
}

// ── Route policies ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePolicy {
    pub name: String,
    pub terms: Vec<PolicyTerm>,
    pub default_action: PolicyAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyTerm {
    pub name: String,
    pub match_conditions: MatchConditions,
    pub actions: Vec<PolicySetAction>,
    pub terminal_action: PolicyAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MatchConditions {
    pub prefix_list: Option<Vec<String>>,
    pub as_path_regex: Option<String>,
    pub community: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicySetAction {
    SetLocalPref(u32),
    SetMed(u32),
    PrependAsPath { asn: u32, count: u32 },
    AddCommunity(String),
    RemoveCommunity(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyAction {
    Accept,
    Reject,
}

// ── Topology container ──

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Topology {
    pub autonomous_systems: HashMap<String, AutonomousSystem>,
    pub standalone_routers: HashMap<String, Router>,
    pub links: HashMap<String, Link>,
    pub policies: HashMap<String, RoutePolicy>,
}

impl Topology {
    pub fn get_router(&self, router_id: &str) -> Option<&Router> {
        if let Some(r) = self.standalone_routers.get(router_id) {
            return Some(r);
        }
        for asys in self.autonomous_systems.values() {
            if let Some(r) = asys.routers.get(router_id) {
                return Some(r);
            }
        }
        None
    }

    pub fn get_router_mut(&mut self, router_id: &str) -> Option<&mut Router> {
        if let Some(r) = self.standalone_routers.get_mut(router_id) {
            return Some(r);
        }
        for asys in self.autonomous_systems.values_mut() {
            if let Some(r) = asys.routers.get_mut(router_id) {
                return Some(r);
            }
        }
        None
    }

    pub fn all_routers(&self) -> Vec<&Router> {
        self.standalone_routers
            .values()
            .chain(
                self.autonomous_systems
                    .values()
                    .flat_map(|a| a.routers.values()),
            )
            .collect()
    }

    pub fn all_routers_mut(&mut self) -> Vec<&mut Router> {
        self.standalone_routers
            .values_mut()
            .chain(
                self.autonomous_systems
                    .values_mut()
                    .flat_map(|a| a.routers.values_mut()),
            )
            .collect()
    }

    pub fn remove_router(&mut self, router_id: &str) -> Option<Router> {
        if let Some(r) = self.standalone_routers.remove(router_id) {
            return Some(r);
        }
        for asys in self.autonomous_systems.values_mut() {
            if let Some(r) = asys.routers.remove(router_id) {
                return Some(r);
            }
        }
        None
    }
}
