use std::collections::HashMap;
use std::net::Ipv4Addr;

use ipnetwork::Ipv4Network;

use crate::engine::models::*;

/// Save topology to JSON string.
pub fn save_topology(topology: &Topology) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(topology)
}

/// Load topology from JSON string.
pub fn load_topology(json: &str) -> Result<Topology, serde_json::Error> {
    serde_json::from_str(json)
}

/// Helper: create an AS with basic defaults.
pub fn create_autonomous_system(asn: u32, name: &str) -> AutonomousSystem {
    AutonomousSystem {
        id: format!("as-{}", asn),
        asn,
        name: name.to_string(),
        routers: HashMap::new(),
        ospf_areas: HashMap::new(),
    }
}

/// Helper: create a router with OSPF enabled in area 0.
pub fn create_router(
    name: &str,
    as_id: &str,
    router_id_ip: Ipv4Addr,
    position: (f64, f64),
) -> Router {
    let router_id = format!("rtr-{}", name.to_lowercase().replace(' ', "-"));

    let mut ospf_areas = HashMap::new();
    ospf_areas.insert(
        0,
        OspfArea {
            area_id: 0,
            interface_ids: Vec::new(),
            is_backbone: true,
            area_type: OspfAreaType::Normal,
        },
    );

    Router {
        id: router_id,
        as_id: as_id.to_string(),
        name: name.to_string(),
        router_id_ip,
        interfaces: HashMap::new(),
        ospf_process: Some(OspfProcess {
            router_id: router_id_ip,
            areas: ospf_areas,
            lsdb: HashMap::new(),
            neighbors: HashMap::new(),
            spf_pending: true,
        }),
        bgp_process: None,
        rib: HashMap::new(),
        fib: HashMap::new(),
        traffic_generators: Vec::new(),
        position,
    }
}

/// Helper: add an interface to a router.
pub fn add_interface(
    router: &mut Router,
    iface_name: &str,
    ip_address: Ipv4Network,
    bandwidth: u64,
    cost: u32,
) -> String {
    let iface_id = format!("{}-{}", router.id, iface_name);

    let iface = Interface {
        id: iface_id.clone(),
        router_id: router.id.clone(),
        ip_address,
        link_id: None,
        bandwidth,
        cost,
        is_up: true,
    };

    router.interfaces.insert(iface_id.clone(), iface);

    // Add to OSPF area 0
    if let Some(ospf) = router.ospf_process.as_mut() {
        if let Some(area) = ospf.areas.get_mut(&0) {
            area.interface_ids.push(iface_id.clone());
        }
    }

    // Add connected route to RIB
    let prefix_with_mask = format!("{}/{}", ip_address.network(), ip_address.prefix());
    router.rib.insert(
        prefix_with_mask.clone(),
        RibEntry {
            prefix: prefix_with_mask,
            next_hop: ip_address.ip(),
            protocol: RouteProtocol::Connected,
            metric: 0,
            admin_distance: 0,
            bgp_attributes: None,
        },
    );

    iface_id
}

/// Helper: create a link between two interfaces.
pub fn create_link(
    topology: &mut Topology,
    iface_a_id: &str,
    iface_b_id: &str,
    bandwidth: u64,
    delay_ms: f64,
) -> String {
    let link_id = format!("link-{}-{}", iface_a_id, iface_b_id);

    let link = Link {
        id: link_id.clone(),
        interface_a_id: iface_a_id.to_string(),
        interface_b_id: iface_b_id.to_string(),
        bandwidth,
        current_load: 0.0,
        delay_ms,
        is_up: true,
    };

    topology.links.insert(link_id.clone(), link);

    // Set link_id on both interfaces (search AS-bound and standalone routers)
    let all_routers = topology.standalone_routers.values_mut().chain(
        topology
            .autonomous_systems
            .values_mut()
            .flat_map(|a| a.routers.values_mut()),
    );
    for router in all_routers {
        if let Some(iface) = router.interfaces.get_mut(iface_a_id) {
            iface.link_id = Some(link_id.clone());
        }
        if let Some(iface) = router.interfaces.get_mut(iface_b_id) {
            iface.link_id = Some(link_id.clone());
        }
    }

    link_id
}

/// Helper: enable BGP on a router.
pub fn enable_bgp(router: &mut Router, local_asn: u32) {
    router.bgp_process = Some(BgpProcess {
        local_asn,
        router_id: router.router_id_ip,
        neighbors: HashMap::new(),
        loc_rib: HashMap::new(),
        best_routes: HashMap::new(),
    });
}

/// Helper: add a BGP neighbor.
pub fn add_bgp_neighbor(
    router: &mut Router,
    neighbor_ip: Ipv4Addr,
    remote_asn: u32,
    local_asn: u32,
    import_policy: Option<String>,
    export_policy: Option<String>,
) {
    if let Some(bgp) = router.bgp_process.as_mut() {
        let is_ebgp = remote_asn != local_asn;
        let key = format!("{}", neighbor_ip);
        bgp.neighbors.insert(
            key,
            BgpNeighbor {
                neighbor_ip,
                remote_asn,
                state: BgpSessionState::Idle,
                is_ebgp,
                adj_rib_in: HashMap::new(),
                adj_rib_out: HashMap::new(),
                import_policy,
                export_policy,
                uptime_ticks: 0,
            },
        );
    }
}

/// Create a sample topology with 2 ASes for testing.
pub fn create_sample_topology() -> Topology {
    let mut topology = Topology::default();

    // AS 65001
    let mut as1 = create_autonomous_system(65001, "AS-Alpha");
    let mut r1 = create_router("R1", &as1.id, "1.1.1.1".parse().unwrap(), (100.0, 200.0));
    let mut r2 = create_router("R2", &as1.id, "2.2.2.2".parse().unwrap(), (300.0, 200.0));
    let mut r3 = create_router("R3", &as1.id, "3.3.3.3".parse().unwrap(), (200.0, 100.0));

    // Interfaces for AS1
    let r1_eth0 = add_interface(
        &mut r1,
        "eth0",
        "10.0.12.1/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r2_eth0 = add_interface(
        &mut r2,
        "eth0",
        "10.0.12.2/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r1_eth1 = add_interface(
        &mut r1,
        "eth1",
        "10.0.13.1/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r3_eth0 = add_interface(
        &mut r3,
        "eth0",
        "10.0.13.2/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r2_eth1 = add_interface(
        &mut r2,
        "eth1",
        "10.0.23.1/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r3_eth1 = add_interface(
        &mut r3,
        "eth1",
        "10.0.23.2/30".parse().unwrap(),
        1_000_000_000,
        10,
    );

    // Loopbacks
    add_interface(&mut r1, "lo0", "10.1.1.1/32".parse().unwrap(), 0, 0);
    add_interface(&mut r2, "lo0", "10.1.2.1/32".parse().unwrap(), 0, 0);
    add_interface(&mut r3, "lo0", "10.1.3.1/32".parse().unwrap(), 0, 0);

    // eBGP interface on R1
    let r1_eth2 = add_interface(
        &mut r1,
        "eth2",
        "172.16.0.1/30".parse().unwrap(),
        1_000_000_000,
        10,
    );

    // Enable BGP on border router
    enable_bgp(&mut r1, 65001);

    as1.routers.insert(r1.id.clone(), r1);
    as1.routers.insert(r2.id.clone(), r2);
    as1.routers.insert(r3.id.clone(), r3);
    topology.autonomous_systems.insert(as1.id.clone(), as1);

    // AS 65002
    let mut as2 = create_autonomous_system(65002, "AS-Beta");
    let mut r4 = create_router("R4", &as2.id, "4.4.4.4".parse().unwrap(), (500.0, 200.0));
    let mut r5 = create_router("R5", &as2.id, "5.5.5.5".parse().unwrap(), (700.0, 200.0));

    let r4_eth0 = add_interface(
        &mut r4,
        "eth0",
        "172.16.0.2/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r4_eth1 = add_interface(
        &mut r4,
        "eth1",
        "10.2.45.1/30".parse().unwrap(),
        1_000_000_000,
        10,
    );
    let r5_eth0 = add_interface(
        &mut r5,
        "eth0",
        "10.2.45.2/30".parse().unwrap(),
        1_000_000_000,
        10,
    );

    add_interface(&mut r4, "lo0", "10.2.4.1/32".parse().unwrap(), 0, 0);
    add_interface(&mut r5, "lo0", "10.2.5.1/32".parse().unwrap(), 0, 0);

    enable_bgp(&mut r4, 65002);

    as2.routers.insert(r4.id.clone(), r4);
    as2.routers.insert(r5.id.clone(), r5);
    topology.autonomous_systems.insert(as2.id.clone(), as2);

    // Create links within AS1
    create_link(&mut topology, &r1_eth0, &r2_eth0, 1_000_000_000, 1.0);
    create_link(&mut topology, &r1_eth1, &r3_eth0, 1_000_000_000, 1.0);
    create_link(&mut topology, &r2_eth1, &r3_eth1, 1_000_000_000, 1.0);

    // Create link within AS2
    create_link(&mut topology, &r4_eth1, &r5_eth0, 1_000_000_000, 1.0);

    // Create eBGP link between AS1 and AS2
    create_link(&mut topology, &r1_eth2, &r4_eth0, 1_000_000_000, 5.0);

    // Set up BGP peering
    if let Some(r1) = topology.get_router_mut("rtr-r1") {
        add_bgp_neighbor(r1, "4.4.4.4".parse().unwrap(), 65002, 65001, None, None);
    }
    if let Some(r4) = topology.get_router_mut("rtr-r4") {
        add_bgp_neighbor(r4, "1.1.1.1".parse().unwrap(), 65001, 65002, None, None);
    }

    topology
}
