use std::collections::HashMap;

use super::models::*;

/// Compute traffic flows from all active generators by walking the FIB.
pub fn compute_flows(topology: &Topology) -> Vec<TrafficFlow> {
    let mut flows = Vec::new();

    for router in topology.all_routers() {
        for gen in &router.traffic_generators {
            if !gen.is_active {
                continue;
            }

            let flow = trace_flow(topology, &router.id, &gen.dest_prefix, gen.id.clone(), gen.rate_bps);
            flows.push(flow);
        }
    }

    flows
}

/// Trace a traffic flow from source router to destination by walking FIBs hop-by-hop.
fn trace_flow(
    topology: &Topology,
    source_router_id: &str,
    dest_prefix: &str,
    generator_id: String,
    rate_bps: u64,
) -> TrafficFlow {
    let mut path: Vec<String> = Vec::new();
    let mut visited: Vec<String> = Vec::new();
    let mut current_router_id = source_router_id.to_string();

    const MAX_HOPS: usize = 64;

    for _ in 0..MAX_HOPS {
        if visited.contains(&current_router_id) {
            // Routing loop detected
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        }
        visited.push(current_router_id.clone());

        let Some(router) = topology.get_router(&current_router_id) else {
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        };

        // Look up the destination in the FIB (longest prefix match)
        let fib_entry = longest_prefix_match(&router.fib, dest_prefix);

        let Some(fib_entry) = fib_entry else {
            // No route — blackholed
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        };

        // Find the outgoing interface and its link
        let Some(out_iface) = router.interfaces.get(&fib_entry.out_interface_id) else {
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        };

        let Some(ref link_id) = out_iface.link_id else {
            // Interface not connected to a link — destination is directly connected
            // Check if the destination prefix is on this interface
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: false,
            };
        };

        // Add this link to the path
        path.push(link_id.clone());

        // Check if the link is up
        let Some(link) = topology.links.get(link_id) else {
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        };

        if !link.is_up {
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        }

        // Find the next router via the other end of the link
        let remote_iface_id = if link.interface_a_id == out_iface.id {
            &link.interface_b_id
        } else {
            &link.interface_a_id
        };

        // Find which router owns the remote interface
        let next_router = topology
            .all_routers()
            .into_iter()
            .find(|r| r.interfaces.contains_key(remote_iface_id));

        let Some(next) = next_router else {
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: true,
            };
        };

        // Check if the destination is on this router (connected route)
        if has_connected_prefix(next, dest_prefix) {
            return TrafficFlow {
                generator_id,
                path,
                rate_bps,
                is_blackholed: false,
            };
        }

        current_router_id = next.id.clone();
    }

    // Max hops exceeded
    TrafficFlow {
        generator_id,
        path,
        rate_bps,
        is_blackholed: true,
    }
}

/// Simple longest-prefix match against the FIB.
fn longest_prefix_match<'a>(
    fib: &'a HashMap<String, FibEntry>,
    dest_prefix: &str,
) -> Option<&'a FibEntry> {
    // Parse destination as IP network
    let dest: ipnetwork::Ipv4Network = match dest_prefix.parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    let dest_ip = dest.network();
    let mut best_match: Option<(&FibEntry, u8)> = None;

    for entry in fib.values() {
        let Ok(fib_prefix) = entry.prefix.parse::<ipnetwork::Ipv4Network>() else {
            continue;
        };

        if fib_prefix.contains(dest_ip) {
            let prefix_len = fib_prefix.prefix();
            match best_match {
                Some((_, best_len)) if prefix_len > best_len => {
                    best_match = Some((entry, prefix_len));
                }
                None => {
                    best_match = Some((entry, prefix_len));
                }
                _ => {}
            }
        }
    }

    best_match.map(|(entry, _)| entry)
}

/// Check if a router has a connected interface matching the destination prefix.
fn has_connected_prefix(router: &Router, dest_prefix: &str) -> bool {
    let Ok(dest) = dest_prefix.parse::<ipnetwork::Ipv4Network>() else {
        return false;
    };

    router.interfaces.values().any(|iface| {
        iface.is_up
            && iface.ip_address.network() == dest.network()
    })
}

/// Update link utilization based on computed flows.
pub fn update_link_utilization(topology: &mut Topology, flows: &[TrafficFlow]) {
    // Reset all link loads
    for link in topology.links.values_mut() {
        link.current_load = 0.0;
    }

    // Sum up traffic on each link
    for flow in flows {
        if flow.is_blackholed {
            continue;
        }
        for link_id in &flow.path {
            if let Some(link) = topology.links.get_mut(link_id) {
                link.current_load += flow.rate_bps as f64;
            }
        }
    }
}

/// Get link utilization as a fraction (0.0 to 1.0+).
pub fn get_link_utilization(topology: &Topology) -> HashMap<String, f64> {
    topology
        .links
        .iter()
        .map(|(id, link)| {
            let util = if link.bandwidth > 0 {
                link.current_load / link.bandwidth as f64
            } else {
                0.0
            };
            (id.clone(), util)
        })
        .collect()
}
