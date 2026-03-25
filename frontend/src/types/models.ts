// Mirrors backend engine/models.rs

export interface Interface {
  id: string;
  router_id: string;
  ip_address: string;
  link_id: string | null;
  bandwidth: number;
  cost: number;
  is_up: boolean;
}

export interface Link {
  id: string;
  interface_a_id: string;
  interface_b_id: string;
  bandwidth: number;
  current_load: number;
  delay_ms: number;
  is_up: boolean;
}

export interface Router {
  id: string;
  as_id: string;
  name: string;
  router_id_ip: string;
  interfaces: Record<string, Interface>;
  ospf_process: OspfProcess | null;
  bgp_process: BgpProcess | null;
  rib: Record<string, RibEntry>;
  fib: Record<string, FibEntry>;
  traffic_generators: TrafficGenerator[];
  position: [number, number];
}

export interface AutonomousSystem {
  id: string;
  asn: number;
  name: string;
  routers: Record<string, Router>;
  ospf_areas: Record<number, string[]>;
}

export interface RibEntry {
  prefix: string;
  next_hop: string;
  protocol: "Connected" | "Ospf" | "BgpExternal" | "BgpInternal";
  metric: number;
  admin_distance: number;
  bgp_attributes: BgpAttributes | null;
}

export interface FibEntry {
  prefix: string;
  next_hop_ip: string;
  out_interface_id: string;
}

// OSPF

export interface OspfProcess {
  router_id: string;
  areas: Record<number, OspfArea>;
  lsdb: Record<string, Lsa>;
  neighbors: Record<string, OspfNeighbor>;
  spf_pending: boolean;
}

export interface OspfArea {
  area_id: number;
  interface_ids: string[];
  is_backbone: boolean;
  area_type: "Normal" | "Stub" | "Nssa";
}

export interface OspfNeighbor {
  neighbor_id: string;
  state: string;
  interface_id: string;
}

export interface Lsa {
  lsa_type: string;
  advertising_router: string;
  link_state_id: string;
  sequence_number: number;
  age: number;
}

// BGP

export interface BgpProcess {
  local_asn: number;
  router_id: string;
  neighbors: Record<string, BgpNeighbor>;
  loc_rib: Record<string, BgpRoute[]>;
  best_routes: Record<string, BgpRoute>;
}

export interface BgpNeighbor {
  neighbor_ip: string;
  remote_asn: number;
  state: "Idle" | "Connect" | "OpenSent" | "OpenConfirm" | "Established";
  is_ebgp: boolean;
  adj_rib_in: Record<string, BgpRoute>;
  adj_rib_out: Record<string, BgpRoute>;
  import_policy: string | null;
  export_policy: string | null;
  uptime_ticks: number;
}

export interface BgpRoute {
  prefix: string;
  attributes: BgpAttributes;
  received_from: string | null;
  is_best: boolean;
}

export interface BgpAttributes {
  as_path: number[];
  local_pref: number;
  med: number;
  next_hop: string;
  origin: "Igp" | "Egp" | "Incomplete";
  communities: string[];
  weight: number;
}

// Traffic

export interface TrafficGenerator {
  id: string;
  source_router_id: string;
  dest_prefix: string;
  rate_bps: number;
  is_active: boolean;
}

export interface TrafficFlow {
  generator_id: string;
  path: string[];
  rate_bps: number;
  is_blackholed: boolean;
}

// Route policies

export interface RoutePolicy {
  name: string;
  terms: PolicyTerm[];
  default_action: "Accept" | "Reject";
}

export interface PolicyTerm {
  name: string;
  match_conditions: MatchConditions;
  actions: PolicySetAction[];
  terminal_action: "Accept" | "Reject";
}

export interface MatchConditions {
  prefix_list: string[] | null;
  as_path_regex: string | null;
  community: string | null;
}

export type PolicySetAction =
  | { SetLocalPref: number }
  | { SetMed: number }
  | { PrependAsPath: { asn: number; count: number } }
  | { AddCommunity: string }
  | { RemoveCommunity: string };

// Simulation state

export interface SimulationState {
  tick: number;
  running: boolean;
  tick_rate_ms: number;
}

// WebSocket tick update (diff)

export interface TickUpdate {
  type: "tick_update";
  tick: number;
  changes: {
    link_utilization: Record<string, number>;
    ospf_converged: string[];
    bgp_state_changes: { router: string; neighbor: string; state: string }[];
    route_changes: { router: string; prefix: string; action: string; via: string }[];
    traffic_flows: TrafficFlow[];
  };
}

// Topology snapshot (full state from REST)

export interface TopologySnapshot {
  autonomous_systems: Record<string, AutonomousSystem>;
  standalone_routers: Record<string, Router>;
  links: Record<string, Link>;
  policies: Record<string, RoutePolicy>;
}
