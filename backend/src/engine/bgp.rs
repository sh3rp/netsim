use std::collections::HashMap;
use std::net::Ipv4Addr;

use super::events::{BgpFsmEvent, EventQueue};
use super::models::*;
use super::ospf::rebuild_fib;

const BGP_FSM_TICKS_TO_ESTABLISHED: u64 = 5;

/// Advance BGP FSM for a neighbor.
pub fn advance_bgp_fsm(state: &BgpSessionState, _event: &BgpFsmEvent) -> BgpSessionState {
    match state {
        BgpSessionState::Idle => BgpSessionState::Connect,
        BgpSessionState::Connect => BgpSessionState::OpenSent,
        BgpSessionState::OpenSent => BgpSessionState::OpenConfirm,
        BgpSessionState::OpenConfirm => BgpSessionState::Established,
        BgpSessionState::Established => BgpSessionState::Established,
    }
}

/// BGP best path selection per RFC 4271 + common extensions.
/// Returns the index of the best route in the candidates list.
pub fn select_best_path(candidates: &[BgpRoute]) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(0);
    }

    let mut best_idx = 0;

    for i in 1..candidates.len() {
        if is_preferred(&candidates[i], &candidates[best_idx]) {
            best_idx = i;
        }
    }

    Some(best_idx)
}

/// Returns true if `a` is preferred over `b`.
fn is_preferred(a: &BgpRoute, b: &BgpRoute) -> bool {
    let aa = &a.attributes;
    let ba = &b.attributes;

    // 1. Highest weight
    if aa.weight != ba.weight {
        return aa.weight > ba.weight;
    }

    // 2. Highest LOCAL_PREF
    if aa.local_pref != ba.local_pref {
        return aa.local_pref > ba.local_pref;
    }

    // 3. Locally originated (no received_from) preferred
    let a_local = a.received_from.is_none();
    let b_local = b.received_from.is_none();
    if a_local != b_local {
        return a_local;
    }

    // 4. Shortest AS_PATH
    if aa.as_path.len() != ba.as_path.len() {
        return aa.as_path.len() < ba.as_path.len();
    }

    // 5. Lowest origin type (IGP < EGP < Incomplete)
    if aa.origin != ba.origin {
        return aa.origin < ba.origin;
    }

    // 6. Lowest MED (only compared when from same neighbor AS)
    let a_neighbor_as = aa.as_path.first();
    let b_neighbor_as = ba.as_path.first();
    if a_neighbor_as == b_neighbor_as && aa.med != ba.med {
        return aa.med < ba.med;
    }

    // 7. eBGP preferred over iBGP
    // (determined by whether as_path contains local ASN at position 0)
    // We approximate: shorter as_path with different first AS = eBGP
    // This is handled at the neighbor level, we'd need the local ASN
    // For now, skip (handled by caller context)

    // 8. Lowest IGP metric to next-hop (would need OSPF cost lookup)
    // Skip for now

    // 9. Lowest router-id tiebreaker
    if let (Some(a_from), Some(b_from)) = (a.received_from, b.received_from) {
        return a_from < b_from;
    }

    false
}

/// Process BGP for all routers in a single tick.
pub fn tick_bgp(topology: &mut Topology, current_tick: u64, event_queue: &mut EventQueue) {
    // Phase 1: Advance FSMs for all BGP neighbors
    advance_all_fsms(topology, current_tick, event_queue);

    // Phase 2: Process route updates for established sessions
    process_route_updates(topology);

    // Phase 3: Run best path selection
    run_best_path_selection(topology);

    // Phase 4: Advertise routes to neighbors
    advertise_routes(topology);
}

fn advance_all_fsms(topology: &mut Topology, current_tick: u64, _event_queue: &mut EventQueue) {
    let mut fsm_updates: Vec<(String, String, BgpSessionState)> = Vec::new();

    for router in topology.all_routers() {
        let Some(bgp) = &router.bgp_process else {
            continue;
        };
        for (key, neighbor) in &bgp.neighbors {
            if neighbor.state != BgpSessionState::Established {
                // Auto-advance FSM each tick until established
                let new_state = advance_bgp_fsm(&neighbor.state, &BgpFsmEvent::Start);
                fsm_updates.push((router.id.clone(), key.clone(), new_state));
            }
        }
    }

    for (router_id, neighbor_key, new_state) in fsm_updates {
        if let Some(router) = topology.get_router_mut(&router_id) {
            if let Some(bgp) = router.bgp_process.as_mut() {
                if let Some(neighbor) = bgp.neighbors.get_mut(&neighbor_key) {
                    neighbor.state = new_state;
                    neighbor.uptime_ticks = current_tick;
                }
            }
        }
    }
}

fn process_route_updates(topology: &mut Topology) {
    // For each router with established BGP sessions, import routes from adj_rib_in to loc_rib
    let router_ids: Vec<String> = topology
        .all_routers()
        .iter()
        .filter(|r| r.bgp_process.is_some())
        .map(|r| r.id.clone())
        .collect();

    for router_id in router_ids {
        let routes_to_import: Vec<(String, BgpRoute)> = {
            let Some(router) = topology.get_router(&router_id) else {
                continue;
            };
            let Some(bgp) = &router.bgp_process else {
                continue;
            };
            let mut routes = Vec::new();
            for neighbor in bgp.neighbors.values() {
                if neighbor.state != BgpSessionState::Established {
                    continue;
                }
                for (prefix, route) in &neighbor.adj_rib_in {
                    routes.push((prefix.clone(), route.clone()));
                }
            }
            routes
        };

        if let Some(router) = topology.get_router_mut(&router_id) {
            if let Some(bgp) = router.bgp_process.as_mut() {
                for (prefix, route) in routes_to_import {
                    bgp.loc_rib.entry(prefix).or_default().push(route);
                }
            }
        }
    }
}

fn run_best_path_selection(topology: &mut Topology) {
    let router_ids: Vec<String> = topology
        .all_routers()
        .iter()
        .filter(|r| r.bgp_process.is_some())
        .map(|r| r.id.clone())
        .collect();

    for router_id in router_ids {
        let best_routes: HashMap<String, BgpRoute> = {
            let Some(router) = topology.get_router(&router_id) else {
                continue;
            };
            let Some(bgp) = &router.bgp_process else {
                continue;
            };
            let mut bests = HashMap::new();
            for (prefix, candidates) in &bgp.loc_rib {
                if let Some(idx) = select_best_path(candidates) {
                    let mut best = candidates[idx].clone();
                    best.is_best = true;
                    bests.insert(prefix.clone(), best);
                }
            }
            bests
        };

        // Install best BGP routes into RIB
        if let Some(router) = topology.get_router_mut(&router_id) {
            // Remove old BGP routes
            router.rib.retain(|_, e| {
                e.protocol != RouteProtocol::BgpExternal
                    && e.protocol != RouteProtocol::BgpInternal
            });

            let local_asn = router
                .bgp_process
                .as_ref()
                .map(|b| b.local_asn)
                .unwrap_or(0);

            for (prefix, route) in &best_routes {
                let is_ebgp = route
                    .attributes
                    .as_path
                    .first()
                    .map_or(false, |first_as| *first_as != local_asn);

                let protocol = if is_ebgp {
                    RouteProtocol::BgpExternal
                } else {
                    RouteProtocol::BgpInternal
                };

                let rib_entry = RibEntry {
                    prefix: prefix.clone(),
                    next_hop: route.attributes.next_hop,
                    protocol: protocol.clone(),
                    metric: route.attributes.med,
                    admin_distance: protocol.admin_distance(),
                    bgp_attributes: Some(route.attributes.clone()),
                };

                // Only install if better than existing
                let should_install = match router.rib.get(prefix) {
                    Some(existing) => rib_entry.admin_distance < existing.admin_distance,
                    None => true,
                };

                if should_install {
                    router.rib.insert(prefix.clone(), rib_entry);
                }
            }

            if let Some(bgp) = router.bgp_process.as_mut() {
                bgp.best_routes = best_routes;
            }

            rebuild_fib(router);
        }
    }
}

fn advertise_routes(topology: &mut Topology) {
    // Collect best routes from each router to advertise to neighbors
    let mut advertisements: Vec<(String, String, String, BgpRoute)> = Vec::new();
    // (target_router_id, target_neighbor_key, prefix, route)

    for asys in topology.autonomous_systems.values() {
        for router in asys.routers.values() {
            let Some(bgp) = &router.bgp_process else {
                continue;
            };

            for (prefix, best_route) in &bgp.best_routes {
                for (nkey, neighbor) in &bgp.neighbors {
                    if neighbor.state != BgpSessionState::Established {
                        continue;
                    }

                    let mut adv_route = best_route.clone();

                    if neighbor.is_ebgp {
                        // eBGP: prepend local ASN to AS-PATH
                        adv_route.attributes.as_path.insert(0, bgp.local_asn);
                        // Set next-hop to local router
                        adv_route.attributes.next_hop = router.router_id_ip;
                    }

                    // Don't re-advertise a route back to the router that sent it
                    if adv_route.received_from == Some(neighbor.neighbor_ip) {
                        continue;
                    }

                    // Find the target router for this neighbor
                    let target_router_id = find_router_by_ip(topology, neighbor.neighbor_ip);
                    if let Some(target_id) = target_router_id {
                        adv_route.received_from = Some(router.router_id_ip);
                        advertisements.push((
                            target_id,
                            format!("{}", router.router_id_ip),
                            prefix.clone(),
                            adv_route,
                        ));
                    }
                }
            }
        }
    }

    // Apply advertisements
    for (target_router_id, from_neighbor_key, prefix, route) in advertisements {
        if let Some(target) = topology.get_router_mut(&target_router_id) {
            if let Some(bgp) = target.bgp_process.as_mut() {
                if let Some(neighbor) = bgp.neighbors.get_mut(&from_neighbor_key) {
                    neighbor.adj_rib_in.insert(prefix, route);
                }
            }
        }
    }
}

fn find_router_by_ip(topology: &Topology, ip: Ipv4Addr) -> Option<String> {
    for asys in topology.autonomous_systems.values() {
        for router in asys.routers.values() {
            if router.router_id_ip == ip {
                return Some(router.id.clone());
            }
        }
    }
    None
}
