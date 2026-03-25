use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::bgp;
use super::events::*;
use super::models::*;
use super::ospf;
use super::traffic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationState {
    pub tick: u64,
    pub running: bool,
    pub tick_rate_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickUpdate {
    pub tick: u64,
    pub link_utilization: std::collections::HashMap<String, f64>,
    pub ospf_converged: Vec<String>,
    pub bgp_state_changes: Vec<BgpStateChange>,
    pub route_changes: Vec<RouteChange>,
    pub traffic_flows: Vec<TrafficFlow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgpStateChange {
    pub router: String,
    pub neighbor: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteChange {
    pub router: String,
    pub prefix: String,
    pub action: String,
    pub via: String,
}

pub struct SimulationEngine {
    pub topology: Topology,
    pub state: SimulationState,
    pub event_queue: EventQueue,
    pub broadcast_tx: broadcast::Sender<TickUpdate>,
}

impl SimulationEngine {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            topology: Topology::default(),
            state: SimulationState {
                tick: 0,
                running: false,
                tick_rate_ms: 200,
            },
            event_queue: EventQueue::new(),
            broadcast_tx: tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TickUpdate> {
        self.broadcast_tx.subscribe()
    }

    /// Run a single simulation tick.
    pub fn step(&mut self) -> TickUpdate {
        self.state.tick += 1;
        let tick = self.state.tick;

        // Process scheduled events
        let events = self.event_queue.drain_for_tick(tick);
        for event in events {
            self.process_event(event);
        }

        // Update OSPF neighbor states
        ospf::update_ospf_neighbors(&mut self.topology);

        // Run OSPF
        ospf::tick_ospf(&mut self.topology, tick, &mut self.event_queue);

        // Run BGP
        bgp::tick_bgp(&mut self.topology, tick, &mut self.event_queue);

        // Compute traffic flows and update utilization
        let flows = traffic::compute_flows(&self.topology);
        traffic::update_link_utilization(&mut self.topology, &flows);

        // Build tick update
        let link_utilization = traffic::get_link_utilization(&self.topology);

        let update = TickUpdate {
            tick,
            link_utilization,
            ospf_converged: Vec::new(), // TODO: track convergence events
            bgp_state_changes: Vec::new(),
            route_changes: Vec::new(),
            traffic_flows: flows,
        };

        // Broadcast (ignore error if no receivers)
        let _ = self.broadcast_tx.send(update.clone());

        update
    }

    fn process_event(&mut self, event: SimEvent) {
        match event.event_type {
            EventType::LinkStateChange { ref link_id, is_up } => {
                if let Some(link) = self.topology.links.get_mut(link_id) {
                    link.is_up = is_up;
                }
                // Mark OSPF as needing recalculation on affected routers
                let affected_ifaces: Vec<(String, String)> = {
                    if let Some(link) = self.topology.links.get(link_id) {
                        vec![
                            (link.interface_a_id.clone(), link_id.clone()),
                            (link.interface_b_id.clone(), link_id.clone()),
                        ]
                    } else {
                        vec![]
                    }
                };
                for (iface_id, _) in affected_ifaces {
                    for asys in self.topology.autonomous_systems.values_mut() {
                        for router in asys.routers.values_mut() {
                            if router.interfaces.contains_key(&iface_id) {
                                if let Some(iface) = router.interfaces.get_mut(&iface_id) {
                                    iface.is_up = is_up;
                                }
                                if let Some(ospf) = router.ospf_process.as_mut() {
                                    ospf.spf_pending = true;
                                }
                            }
                        }
                    }
                }
            }
            EventType::LsaFlood {
                ref lsa_key,
                ref source_router_id,
                area_id,
            } => {
                ospf::flood_lsa_to_area(&mut self.topology, lsa_key, source_router_id, area_id);
            }
            EventType::BgpSessionEvent {
                ref router_id,
                ref neighbor_ip,
                ref event,
            } => {
                if let Some(router) = self.topology.get_router_mut(router_id) {
                    if let Some(bgp) = router.bgp_process.as_mut() {
                        if let Some(neighbor) = bgp.neighbors.get_mut(neighbor_ip) {
                            neighbor.state = bgp::advance_bgp_fsm(&neighbor.state, event);
                        }
                    }
                }
            }
            EventType::TrafficStart { ref generator_id } => {
                for router in self.topology.all_routers_mut() {
                    for gen in &mut router.traffic_generators {
                        if gen.id == *generator_id {
                            gen.is_active = true;
                        }
                    }
                }
            }
            EventType::TrafficStop { ref generator_id } => {
                for router in self.topology.all_routers_mut() {
                    for gen in &mut router.traffic_generators {
                        if gen.id == *generator_id {
                            gen.is_active = false;
                        }
                    }
                }
            }
            EventType::BgpUpdate { .. } | EventType::PolicyChange { .. } => {
                // Handled by the normal BGP tick processing
            }
        }
    }
}

/// Run the simulation loop in a tokio task.
pub async fn run_simulation_loop(engine: Arc<RwLock<SimulationEngine>>) {
    loop {
        let (running, tick_rate_ms) = {
            let e = engine.read();
            (e.state.running, e.state.tick_rate_ms)
        };

        if running {
            engine.write().step();
        }

        tokio::time::sleep(std::time::Duration::from_millis(tick_rate_ms)).await;
    }
}
