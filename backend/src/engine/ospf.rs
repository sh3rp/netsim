use std::collections::HashMap;
use std::net::Ipv4Addr;

use ipnetwork::Ipv4Network;
use petgraph::algo::dijkstra;
use petgraph::graph::{DiGraph, NodeIndex};

use super::events::{EventQueue, EventType};
use super::models::*;

const LSA_FLOOD_DELAY_TICKS: u64 = 2;

/// Generate a Router LSA (Type-1) for a router in a given area.
pub fn originate_router_lsa(router: &Router, area_id: u32) -> Lsa {
    let ospf = router.ospf_process.as_ref().unwrap();
    let area = &ospf.areas[&area_id];

    let mut links = Vec::new();
    for iface_id in &area.interface_ids {
        if let Some(iface) = router.interfaces.get(iface_id) {
            if !iface.is_up {
                continue;
            }
            // Stub network for the interface's connected subnet
            links.push(RouterLsaLink {
                link_id: format!("{}", iface.ip_address.network()),
                link_data: iface.ip_address.ip(),
                link_type: RouterLsaLinkType::StubNetwork,
                metric: iface.cost,
            });
            // Point-to-point link to neighbor if connected
            if let Some(ref link_id) = iface.link_id {
                links.push(RouterLsaLink {
                    link_id: link_id.clone(),
                    link_data: iface.ip_address.ip(),
                    link_type: RouterLsaLinkType::PointToPoint,
                    metric: iface.cost,
                });
            }
        }
    }

    let seq = ospf
        .lsdb
        .get(&lsa_key_router(router.router_id_ip, area_id))
        .map(|l| l.sequence_number + 1)
        .unwrap_or(1);

    Lsa {
        lsa_type: LsaType::RouterLsa,
        advertising_router: router.router_id_ip,
        link_state_id: format!("{}", router.router_id_ip),
        sequence_number: seq,
        age: 0,
        body: LsaBody::Router { links },
    }
}

/// Install an LSA into a router's LSDB, returns true if it's new/updated.
pub fn install_lsa(router: &mut Router, area_id: u32, lsa: Lsa) -> bool {
    let ospf = router.ospf_process.as_mut().unwrap();
    let key = match lsa.lsa_type {
        LsaType::RouterLsa => lsa_key_router(lsa.advertising_router, area_id),
        LsaType::NetworkLsa => format!("net:{}:{}", lsa.link_state_id, area_id),
        LsaType::SummaryLsa => format!("sum:{}:{}", lsa.link_state_id, area_id),
    };

    let should_install = match ospf.lsdb.get(&key) {
        Some(existing) => lsa.sequence_number > existing.sequence_number,
        None => true,
    };

    if should_install {
        ospf.lsdb.insert(key, lsa);
        ospf.spf_pending = true;
        true
    } else {
        false
    }
}

/// Schedule LSA flooding to all same-area routers.
pub fn schedule_lsa_flood(
    lsa_key: &str,
    source_router_id: &str,
    area_id: u32,
    current_tick: u64,
    event_queue: &mut EventQueue,
) {
    event_queue.schedule(
        current_tick + LSA_FLOOD_DELAY_TICKS,
        10,
        EventType::LsaFlood {
            lsa_key: lsa_key.to_string(),
            source_router_id: source_router_id.to_string(),
            area_id,
        },
    );
}

/// Flood an LSA to all routers in the same area (simplified: direct copy).
pub fn flood_lsa_to_area(topology: &mut Topology, lsa_key: &str, source_router_id: &str, area_id: u32) {
    // First, extract the LSA from the source router
    let lsa = {
        let source = topology.get_router(source_router_id).unwrap();
        let ospf = source.ospf_process.as_ref().unwrap();
        ospf.lsdb.get(lsa_key).cloned()
    };

    let Some(lsa) = lsa else { return };

    // Find all routers in the same area (excluding source)
    let router_ids: Vec<String> = topology
        .all_routers()
        .iter()
        .filter(|r| {
            r.id != source_router_id
                && r.ospf_process.as_ref().map_or(false, |o| o.areas.contains_key(&area_id))
        })
        .map(|r| r.id.clone())
        .collect();

    // Install LSA in each router
    for rid in router_ids {
        if let Some(router) = topology.get_router_mut(&rid) {
            install_lsa(router, area_id, lsa.clone());
        }
    }
}

/// Run Dijkstra SPF on the LSDB for a given router and area.
pub fn run_spf(router: &Router, _area_id: u32) -> Vec<SpfResult> {
    let ospf = router.ospf_process.as_ref().unwrap();

    // Build a directed graph from Router LSAs in this area
    let mut graph = DiGraph::<Ipv4Addr, u32>::new();
    let mut node_map: HashMap<Ipv4Addr, NodeIndex> = HashMap::new();
    let mut prefix_map: HashMap<String, (Ipv4Addr, u32)> = HashMap::new(); // prefix -> (router_id, cost)

    // Collect all router LSAs for this area
    for lsa in ospf.lsdb.values() {
        if lsa.lsa_type != LsaType::RouterLsa {
            continue;
        }
        // Only process LSAs for this area (check key format)
        let router_ip = lsa.advertising_router;
        let _node = *node_map
            .entry(router_ip)
            .or_insert_with(|| graph.add_node(router_ip));

        if let LsaBody::Router { ref links } = lsa.body {
            for link in links {
                match link.link_type {
                    RouterLsaLinkType::PointToPoint => {
                        // link_data is the local IP, link_id is the link ID
                        // We need to find the remote router by link_id
                    }
                    RouterLsaLinkType::StubNetwork => {
                        let entry = prefix_map
                            .entry(link.link_id.clone())
                            .or_insert((router_ip, link.metric));
                        if link.metric < entry.1 {
                            *entry = (router_ip, link.metric);
                        }
                    }
                    RouterLsaLinkType::TransitNetwork => {}
                }
            }
        }
    }

    // Build edges: for each pair of Router LSAs that share a point-to-point link
    let router_lsas: Vec<&Lsa> = ospf
        .lsdb
        .values()
        .filter(|l| l.lsa_type == LsaType::RouterLsa)
        .collect();

    // Map link_id -> Vec<(router_ip, cost, local_ip)>
    let mut link_endpoints: HashMap<String, Vec<(Ipv4Addr, u32, Ipv4Addr)>> = HashMap::new();
    for lsa in &router_lsas {
        if let LsaBody::Router { ref links } = lsa.body {
            for link in links {
                if link.link_type == RouterLsaLinkType::PointToPoint {
                    link_endpoints
                        .entry(link.link_id.clone())
                        .or_default()
                        .push((lsa.advertising_router, link.metric, link.link_data));
                }
            }
        }
    }

    // Add edges for shared links
    for (_link_id, endpoints) in &link_endpoints {
        if endpoints.len() == 2 {
            let (r1, cost1, _) = &endpoints[0];
            let (r2, cost2, _) = &endpoints[1];
            let n1 = *node_map.entry(*r1).or_insert_with(|| graph.add_node(*r1));
            let n2 = *node_map.entry(*r2).or_insert_with(|| graph.add_node(*r2));
            graph.add_edge(n1, n2, *cost1);
            graph.add_edge(n2, n1, *cost2);
        }
    }

    // Run Dijkstra from this router
    let Some(start_node) = node_map.get(&router.router_id_ip) else {
        return Vec::new();
    };

    let costs = dijkstra(&graph, *start_node, None, |e| *e.weight());

    // Build SPF results: for each reachable prefix, determine next-hop
    let mut results = Vec::new();

    // First, build next-hop map by backtracking the shortest path tree
    let next_hops = compute_next_hops(&graph, &node_map, *start_node, &costs, router);

    // For each prefix, find the nearest router and determine next-hop
    for (prefix_str, (owner_ip, stub_cost)) in &prefix_map {
        let Ok(prefix) = prefix_str.parse::<Ipv4Network>() else {
            continue;
        };

        let Some(owner_node) = node_map.get(owner_ip) else {
            continue;
        };
        let Some(cost_to_owner) = costs.get(owner_node) else {
            continue;
        };

        let total_cost = cost_to_owner + stub_cost;

        if *owner_ip == router.router_id_ip {
            // Connected or directly attached
            if let Some((next_hop, iface_id)) = find_local_interface(router, &prefix) {
                results.push(SpfResult {
                    destination: prefix,
                    cost: total_cost,
                    next_hop,
                    next_hop_interface_id: iface_id,
                });
            }
        } else if let Some((nh_ip, nh_iface)) = next_hops.get(owner_ip) {
            results.push(SpfResult {
                destination: prefix,
                cost: total_cost,
                next_hop: *nh_ip,
                next_hop_interface_id: nh_iface.clone(),
            });
        }
    }

    results
}

/// Compute next-hops from the SPF tree.
fn compute_next_hops(
    graph: &DiGraph<Ipv4Addr, u32>,
    node_map: &HashMap<Ipv4Addr, NodeIndex>,
    start: NodeIndex,
    costs: &HashMap<NodeIndex, u32>,
    router: &Router,
) -> HashMap<Ipv4Addr, (Ipv4Addr, String)> {
    let mut next_hops: HashMap<Ipv4Addr, (Ipv4Addr, String)> = HashMap::new();

    // For each destination, trace back to find the first hop
    let reverse_map: HashMap<NodeIndex, Ipv4Addr> = node_map.iter().map(|(ip, n)| (*n, *ip)).collect();

    for (node, _cost) in costs {
        if *node == start {
            continue;
        }
        let dest_ip = reverse_map[node];

        // Walk backwards through the graph to find the first hop from start
        let mut current = *node;
        let mut prev = *node;

        // Simple BFS back-track using costs
        loop {
            let mut found_parent = false;
            for neighbor in graph.neighbors_directed(current, petgraph::Direction::Incoming) {
                if let (Some(nc), Some(cc)) = (costs.get(&neighbor), costs.get(&current)) {
                    let edge_weight = graph
                        .edges_connecting(neighbor, current)
                        .next()
                        .map(|e| *e.weight())
                        .unwrap_or(u32::MAX);
                    if *nc + edge_weight == *cc {
                        prev = current;
                        current = neighbor;
                        found_parent = true;
                        break;
                    }
                }
            }
            if current == start || !found_parent {
                break;
            }
        }

        // `prev` is now the first hop from start
        let first_hop_ip = reverse_map.get(&prev).copied().unwrap_or(dest_ip);

        // Find the interface that reaches this first hop
        if let Some(iface_id) = find_interface_to_neighbor(router, first_hop_ip) {
            next_hops.insert(dest_ip, (first_hop_ip, iface_id));
        }
    }

    next_hops
}

/// Find the local interface that connects to a given prefix.
fn find_local_interface(router: &Router, prefix: &Ipv4Network) -> Option<(Ipv4Addr, String)> {
    for iface in router.interfaces.values() {
        if iface.is_up && iface.ip_address.network() == prefix.network() {
            return Some((iface.ip_address.ip(), iface.id.clone()));
        }
    }
    None
}

/// Find which interface on this router can reach a neighbor router.
fn find_interface_to_neighbor(router: &Router, neighbor_ip: Ipv4Addr) -> Option<String> {
    for iface in router.interfaces.values() {
        if iface.is_up && iface.ip_address.contains(neighbor_ip) {
            return Some(iface.id.clone());
        }
    }
    // Fallback: any interface with a link
    router
        .interfaces
        .values()
        .find(|i| i.is_up && i.link_id.is_some())
        .map(|i| i.id.clone())
}

/// Install OSPF routes into the router's RIB and rebuild FIB.
pub fn install_ospf_routes(router: &mut Router, spf_results: &[SpfResult]) {
    // Remove old OSPF routes from RIB
    router
        .rib
        .retain(|_, entry| entry.protocol != RouteProtocol::Ospf);

    // Install new OSPF routes
    for result in spf_results {
        let prefix_str = format!("{}", result.destination);
        let entry = RibEntry {
            prefix: prefix_str.clone(),
            next_hop: result.next_hop,
            protocol: RouteProtocol::Ospf,
            metric: result.cost,
            admin_distance: RouteProtocol::Ospf.admin_distance(),
            bgp_attributes: None,
        };

        // Only install if better than existing route
        let should_install = match router.rib.get(&prefix_str) {
            Some(existing) => {
                entry.admin_distance < existing.admin_distance
                    || (entry.admin_distance == existing.admin_distance
                        && entry.metric < existing.metric)
            }
            None => true,
        };

        if should_install {
            router.rib.insert(prefix_str, entry);
        }
    }

    rebuild_fib(router);
}

/// Rebuild FIB from RIB (select best route per prefix).
pub fn rebuild_fib(router: &mut Router) {
    router.fib.clear();
    for (prefix, rib_entry) in &router.rib {
        let should_install = match router.fib.get(prefix) {
            Some(_) => false, // RIB already has one entry per prefix after selection
            None => true,
        };
        if should_install {
            // Find the outgoing interface for this next-hop
            let out_iface = router
                .interfaces
                .values()
                .find(|i| i.is_up && i.ip_address.contains(rib_entry.next_hop))
                .map(|i| i.id.clone())
                .unwrap_or_default();

            router.fib.insert(
                prefix.clone(),
                FibEntry {
                    prefix: prefix.clone(),
                    next_hop_ip: rib_entry.next_hop,
                    out_interface_id: out_iface,
                },
            );
        }
    }
}

/// Process OSPF for all routers in a single tick.
pub fn tick_ospf(topology: &mut Topology, current_tick: u64, event_queue: &mut EventQueue) {
    // Phase 1: Originate LSAs for all routers
    let mut lsa_updates: Vec<(String, u32, String, Lsa)> = Vec::new(); // (router_id, area_id, lsa_key, lsa)

    for asys in topology.autonomous_systems.values() {
        for router in asys.routers.values() {
            let Some(ospf) = &router.ospf_process else {
                continue;
            };
            for area_id in ospf.areas.keys() {
                let lsa = originate_router_lsa(router, *area_id);
                let key = lsa_key_router(router.router_id_ip, *area_id);

                // Check if LSA changed
                let changed = ospf
                    .lsdb
                    .get(&key)
                    .map_or(true, |existing| lsa.sequence_number > existing.sequence_number);

                if changed {
                    lsa_updates.push((router.id.clone(), *area_id, key, lsa));
                }
            }
        }
    }

    // Install LSAs and schedule floods
    for (router_id, area_id, key, lsa) in lsa_updates {
        if let Some(router) = topology.get_router_mut(&router_id) {
            install_lsa(router, area_id, lsa);
            schedule_lsa_flood(&key, &router_id, area_id, current_tick, event_queue);
        }
    }

    // Phase 2: Run SPF on routers that need it
    let pending_routers: Vec<String> = topology
        .all_routers()
        .iter()
        .filter(|r| {
            r.ospf_process
                .as_ref()
                .map_or(false, |o| o.spf_pending)
        })
        .map(|r| r.id.clone())
        .collect();

    for router_id in pending_routers {
        let spf_results = {
            let router = topology.get_router(&router_id).unwrap();
            let ospf = router.ospf_process.as_ref().unwrap();
            let mut all_results = Vec::new();
            for area_id in ospf.areas.keys() {
                all_results.extend(run_spf(router, *area_id));
            }
            all_results
        };

        if let Some(router) = topology.get_router_mut(&router_id) {
            install_ospf_routes(router, &spf_results);
            if let Some(ospf) = router.ospf_process.as_mut() {
                ospf.spf_pending = false;
            }
        }
    }
}

/// Update OSPF neighbor states based on link connectivity.
pub fn update_ospf_neighbors(topology: &mut Topology) {
    // Collect link state info
    let link_info: Vec<(String, String, bool)> = topology
        .links
        .values()
        .map(|l| (l.interface_a_id.clone(), l.interface_b_id.clone(), l.is_up))
        .collect();

    for (iface_a_id, iface_b_id, link_up) in &link_info {
        // Find routers for each interface
        let router_a_info = find_router_for_interface(topology, iface_a_id);
        let router_b_info = find_router_for_interface(topology, iface_b_id);

        if let (Some((ra_id, ra_ip)), Some((rb_id, rb_ip))) = (router_a_info, router_b_info) {
            let state = if *link_up {
                OspfNeighborState::Full
            } else {
                OspfNeighborState::Down
            };

            // Set neighbor on router A
            if let Some(router_a) = topology.get_router_mut(&ra_id) {
                if let Some(ospf) = router_a.ospf_process.as_mut() {
                    let neighbor_key = format!("{}", rb_ip);
                    ospf.neighbors.insert(
                        neighbor_key,
                        OspfNeighbor {
                            neighbor_id: rb_ip,
                            state: state.clone(),
                            interface_id: iface_a_id.clone(),
                        },
                    );
                }
            }

            // Set neighbor on router B
            if let Some(router_b) = topology.get_router_mut(&rb_id) {
                if let Some(ospf) = router_b.ospf_process.as_mut() {
                    let neighbor_key = format!("{}", ra_ip);
                    ospf.neighbors.insert(
                        neighbor_key,
                        OspfNeighbor {
                            neighbor_id: ra_ip,
                            state,
                            interface_id: iface_b_id.clone(),
                        },
                    );
                }
            }
        }
    }
}

fn find_router_for_interface(topology: &Topology, iface_id: &str) -> Option<(String, Ipv4Addr)> {
    for asys in topology.autonomous_systems.values() {
        for router in asys.routers.values() {
            if router.interfaces.contains_key(iface_id) {
                return Some((router.id.clone(), router.router_id_ip));
            }
        }
    }
    None
}

fn lsa_key_router(router_ip: Ipv4Addr, area_id: u32) -> String {
    format!("rtr:{}:{}", router_ip, area_id)
}
