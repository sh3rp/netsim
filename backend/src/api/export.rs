use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{web, HttpResponse};
use parking_lot::RwLock;
use serde::Serialize;
use uuid::Uuid;

use crate::engine::models::Topology;
use crate::engine::simulation::SimulationEngine;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/export").route("/gns3", web::get().to(export_gns3)));
}

async fn export_gns3(state: AppState) -> HttpResponse {
    let engine = state.read();
    let gns3 = topology_to_gns3(&engine.topology);

    match serde_json::to_string_pretty(&gns3) {
        Ok(json) => HttpResponse::Ok()
            .content_type("application/json")
            .insert_header((
                "Content-Disposition",
                "attachment; filename=\"netsim-project.gns3\"",
            ))
            .body(json),
        Err(e) => HttpResponse::InternalServerError().json(crate::api::schemas::MessageResponse {
            message: format!("Failed to export: {}", e),
        }),
    }
}

fn topology_to_gns3(topology: &Topology) -> Gns3Project {
    let project_id = Uuid::new_v4().to_string();
    let mut nodes = Vec::new();
    let mut links = Vec::new();

    // Map from our router ID -> GNS3 node UUID
    let mut router_node_map: HashMap<String, String> = HashMap::new();
    // Map from our interface ID -> (gns3_node_id, adapter_number, port_number)
    let mut iface_port_map: HashMap<String, (String, u32, u32)> = HashMap::new();

    // Create GNS3 nodes for all routers (AS-bound and standalone)
    let all_routers: Vec<&crate::engine::models::Router> = topology
        .standalone_routers
        .values()
        .chain(
            topology
                .autonomous_systems
                .values()
                .flat_map(|a| a.routers.values()),
        )
        .collect();

    for router in &all_routers {
        let node_id = Uuid::new_v4().to_string();
        router_node_map.insert(router.id.clone(), node_id.clone());

        // Map interfaces to adapter/port numbers
        let mut ordered_ifaces: Vec<_> = router.interfaces.iter().collect();
        ordered_ifaces.sort_by(|(a, _), (b, _)| a.cmp(b));

        let mut ports = Vec::new();
        for (port_number, (iface_id, iface)) in ordered_ifaces.into_iter().enumerate() {
            let port_number = port_number as u32;
            iface_port_map.insert(iface_id.clone(), (node_id.clone(), 0, port_number));

            ports.push(Gns3Port {
                adapter_number: 0,
                port_number,
                name: format!("Ethernet{}", port_number),
                short_name: format!("e{}", port_number),
                link_type: "ethernet".to_string(),
                data_link_types: HashMap::from([(
                    "Ethernet".to_string(),
                    "DLT_EN10MB".to_string(),
                )]),
                description: format!("{} - {}", iface_id, iface.ip_address),
            });
        }

        // Build router configuration for properties
        let startup_config = build_router_config(router, topology);

        let node = Gns3Node {
            compute_id: "local".to_string(),
            console: None,
            console_auto_start: false,
            console_type: "telnet".to_string(),
            first_port_name: None,
            height: 45,
            width: 66,
            label: Gns3Label {
                rotation: 0,
                style: "font-family: TypeWriter;font-size: 10.0;font-weight: bold;fill: #000000;fill-opacity: 1.0;".to_string(),
                text: router.name.clone(),
                x: 2,
                y: -25,
            },
            name: router.name.clone(),
            node_id: node_id.clone(),
            node_type: "dynamips".to_string(),
            port_name_format: "Ethernet{0}".to_string(),
            port_segment_size: 0,
            ports,
            properties: Gns3NodeProperties {
                platform: "c7200".to_string(),
                ram: 512,
                nvram: 256,
                image: "c7200-adventerprisek9-mz.152-4.M7.image".to_string(),
                startup_config_content: if startup_config.is_empty() {
                    None
                } else {
                    Some(startup_config)
                },
                slot0: Some("C7200-IO-FE".to_string()),
                slot1: Some("PA-GE".to_string()),
            },
            symbol: ":/symbols/router.svg".to_string(),
            x: router.position.0 as i32,
            y: router.position.1 as i32,
            z: 1,
        };

        nodes.push(node);
    }

    // Create GNS3 links
    for link in topology.links.values() {
        let side_a = iface_port_map.get(&link.interface_a_id);
        let side_b = iface_port_map.get(&link.interface_b_id);

        if let (Some((node_a, adapter_a, port_a)), Some((node_b, adapter_b, port_b))) =
            (side_a, side_b)
        {
            let link_id = Uuid::new_v4().to_string();

            let gns3_link = Gns3Link {
                filters: HashMap::new(),
                link_id,
                link_style: HashMap::new(),
                nodes: vec![
                    Gns3LinkNode {
                        adapter_number: *adapter_a,
                        node_id: node_a.clone(),
                        port_number: *port_a,
                        label: Gns3Label {
                            rotation: 0,
                            style: "font-family: TypeWriter;font-size: 10.0;fill: #000000;fill-opacity: 1.0;".to_string(),
                            text: format!("e{}", port_a),
                            x: 0,
                            y: -25,
                        },
                    },
                    Gns3LinkNode {
                        adapter_number: *adapter_b,
                        node_id: node_b.clone(),
                        port_number: *port_b,
                        label: Gns3Label {
                            rotation: 0,
                            style: "font-family: TypeWriter;font-size: 10.0;fill: #000000;fill-opacity: 1.0;".to_string(),
                            text: format!("e{}", port_b),
                            x: 0,
                            y: -25,
                        },
                    },
                ],
                suspend: false,
            };

            links.push(gns3_link);
        }
    }

    // Create drawings for AS boundaries
    let mut drawings = Vec::new();
    for asys in topology.autonomous_systems.values() {
        if asys.routers.is_empty() {
            continue;
        }

        // Compute bounding box of routers in this AS
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for router in asys.routers.values() {
            min_x = min_x.min(router.position.0);
            min_y = min_y.min(router.position.1);
            max_x = max_x.max(router.position.0);
            max_y = max_y.max(router.position.1);
        }

        let padding = 80.0;
        let x = min_x - padding;
        let y = min_y - padding;
        let w = (max_x - min_x) + padding * 2.0;
        let h = (max_y - min_y) + padding * 2.0;

        let w = w as i32;
        let h = h as i32;
        let svg = format!(
            "<svg width=\"{}\" height=\"{}\"><rect width=\"{}\" height=\"{}\" fill=\"#ffffff\" fill-opacity=\"0.1\" stroke=\"#4a90d9\" stroke-width=\"2\" stroke-dasharray=\"10,5\"/><text x=\"10\" y=\"20\" font-family=\"TypeWriter\" font-size=\"14\" fill=\"#4a90d9\">AS {} - {}</text></svg>",
            w, h, w, h, asys.asn, asys.name,
        );

        drawings.push(Gns3Drawing {
            drawing_id: Uuid::new_v4().to_string(),
            x: x as i32,
            y: y as i32,
            z: 0,
            rotation: 0,
            svg,
        });
    }

    Gns3Project {
        auto_close: true,
        auto_open: false,
        auto_start: false,
        drawing_grid_size: 25,
        grid_size: 75,
        name: "netsim-export".to_string(),
        project_id,
        revision: 9,
        scene_height: 1000,
        scene_width: 2000,
        show_grid: false,
        show_interface_labels: true,
        show_layers: false,
        snap_to_grid: false,
        supplier: None,
        topology: Gns3Topology {
            computes: vec![],
            drawings,
            links,
            nodes,
        },
        topology_type: "topology".to_string(),
        version: "2.2.46".to_string(),
        zoom: 100,
    }
}

/// Build a Cisco IOS-style startup config for a router.
fn build_router_config(router: &crate::engine::models::Router, topology: &Topology) -> String {
    let mut config = String::new();
    let mut ordered_ifaces: Vec<_> = router.interfaces.iter().collect();
    ordered_ifaces.sort_by(|(a, _), (b, _)| a.cmp(b));

    config.push_str("!\n");
    config.push_str(&format!("hostname {}\n", router.name));
    config.push_str("!\n");

    // Interfaces
    for (idx, (_iface_id, iface)) in ordered_ifaces.iter().enumerate() {
        config.push_str(&format!("interface Ethernet{}\n", idx));
        config.push_str(&format!(
            " ip address {} {}\n",
            iface.ip_address.ip(),
            prefix_to_mask(iface.ip_address.prefix())
        ));
        if let Some(ref link_id) = iface.link_id {
            if let Some(link) = topology.links.get(link_id) {
                config.push_str(&format!(" bandwidth {}\n", link.bandwidth / 1000));
                // kbps
            }
        }
        if iface.is_up {
            config.push_str(" no shutdown\n");
        } else {
            config.push_str(" shutdown\n");
        }
        config.push_str("!\n");
    }

    // OSPF
    if let Some(ref ospf) = router.ospf_process {
        config.push_str(&format!("router ospf 1\n"));
        config.push_str(&format!(" router-id {}\n", ospf.router_id));

        for (area_id, area) in &ospf.areas {
            for iface_id in &area.interface_ids {
                if let Some(iface) = router.interfaces.get(iface_id) {
                    config.push_str(&format!(
                        " network {} {} area {}\n",
                        iface.ip_address.network(),
                        wildcard_mask(iface.ip_address.prefix()),
                        area_id
                    ));
                }
            }
        }

        // Interface costs
        for (idx, (_iface_id, iface)) in ordered_ifaces.iter().enumerate() {
            if iface.cost != 1 {
                config.push_str(&format!(
                    "interface Ethernet{}\n ip ospf cost {}\n!\n",
                    idx, iface.cost
                ));
            }
        }
        config.push_str("!\n");
    }

    // BGP
    if let Some(ref bgp) = router.bgp_process {
        config.push_str(&format!("router bgp {}\n", bgp.local_asn));
        config.push_str(&format!(" bgp router-id {}\n", bgp.router_id));

        for neighbor in bgp.neighbors.values() {
            config.push_str(&format!(
                " neighbor {} remote-as {}\n",
                neighbor.neighbor_ip, neighbor.remote_asn
            ));

            if let Some(ref policy_name) = neighbor.import_policy {
                config.push_str(&format!(
                    " neighbor {} route-map {} in\n",
                    neighbor.neighbor_ip, policy_name
                ));
            }
            if let Some(ref policy_name) = neighbor.export_policy {
                config.push_str(&format!(
                    " neighbor {} route-map {} out\n",
                    neighbor.neighbor_ip, policy_name
                ));
            }
        }
        config.push_str("!\n");
    }

    // Route maps from policies
    for (policy_name, policy) in &topology.policies {
        config.push_str(&format!("! Policy: {}\n", policy_name));
        for (seq, term) in policy.terms.iter().enumerate() {
            let action = match term.terminal_action {
                crate::engine::models::PolicyAction::Accept => "permit",
                crate::engine::models::PolicyAction::Reject => "deny",
            };
            config.push_str(&format!(
                "route-map {} {} {}\n",
                policy_name,
                action,
                (seq + 1) * 10
            ));

            for set_action in &term.actions {
                match set_action {
                    crate::engine::models::PolicySetAction::SetLocalPref(v) => {
                        config.push_str(&format!(" set local-preference {}\n", v));
                    }
                    crate::engine::models::PolicySetAction::SetMed(v) => {
                        config.push_str(&format!(" set metric {}\n", v));
                    }
                    crate::engine::models::PolicySetAction::PrependAsPath { asn, count } => {
                        let prepend: Vec<String> = (0..*count).map(|_| asn.to_string()).collect();
                        config.push_str(&format!(" set as-path prepend {}\n", prepend.join(" ")));
                    }
                    crate::engine::models::PolicySetAction::AddCommunity(c) => {
                        config.push_str(&format!(" set community {} additive\n", c));
                    }
                    crate::engine::models::PolicySetAction::RemoveCommunity(c) => {
                        config.push_str(&format!(" set comm-list {} delete\n", c));
                    }
                }
            }
            config.push_str("!\n");
        }
    }

    config.push_str("end\n");
    config
}

fn prefix_to_mask(prefix_len: u8) -> String {
    if prefix_len == 0 {
        return "0.0.0.0".to_string();
    }
    let mask: u32 = !0u32 << (32 - prefix_len);
    format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xFF,
        (mask >> 16) & 0xFF,
        (mask >> 8) & 0xFF,
        mask & 0xFF
    )
}

fn wildcard_mask(prefix_len: u8) -> String {
    let mask: u32 = !0u32 << (32 - prefix_len);
    let wildcard = !mask;
    format!(
        "{}.{}.{}.{}",
        (wildcard >> 24) & 0xFF,
        (wildcard >> 16) & 0xFF,
        (wildcard >> 8) & 0xFF,
        wildcard & 0xFF
    )
}

// ── GNS3 project file structures ──

#[derive(Serialize)]
struct Gns3Project {
    auto_close: bool,
    auto_open: bool,
    auto_start: bool,
    drawing_grid_size: u32,
    grid_size: u32,
    name: String,
    project_id: String,
    revision: u32,
    scene_height: u32,
    scene_width: u32,
    show_grid: bool,
    show_interface_labels: bool,
    show_layers: bool,
    snap_to_grid: bool,
    supplier: Option<String>,
    topology: Gns3Topology,
    #[serde(rename = "type")]
    topology_type: String,
    version: String,
    zoom: u32,
}

#[derive(Serialize)]
struct Gns3Topology {
    computes: Vec<serde_json::Value>,
    drawings: Vec<Gns3Drawing>,
    links: Vec<Gns3Link>,
    nodes: Vec<Gns3Node>,
}

#[derive(Serialize)]
struct Gns3Node {
    compute_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    console: Option<u16>,
    console_auto_start: bool,
    console_type: String,
    first_port_name: Option<String>,
    height: u32,
    width: u32,
    label: Gns3Label,
    name: String,
    node_id: String,
    node_type: String,
    port_name_format: String,
    port_segment_size: u32,
    ports: Vec<Gns3Port>,
    properties: Gns3NodeProperties,
    symbol: String,
    x: i32,
    y: i32,
    z: u32,
}

#[derive(Serialize)]
struct Gns3NodeProperties {
    platform: String,
    ram: u32,
    nvram: u32,
    image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    startup_config_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot0: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot1: Option<String>,
}

#[derive(Serialize)]
struct Gns3Port {
    adapter_number: u32,
    port_number: u32,
    name: String,
    short_name: String,
    link_type: String,
    data_link_types: HashMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
}

#[derive(Serialize)]
struct Gns3Link {
    filters: HashMap<String, serde_json::Value>,
    link_id: String,
    link_style: HashMap<String, serde_json::Value>,
    nodes: Vec<Gns3LinkNode>,
    suspend: bool,
}

#[derive(Serialize)]
struct Gns3LinkNode {
    adapter_number: u32,
    node_id: String,
    port_number: u32,
    label: Gns3Label,
}

#[derive(Serialize)]
struct Gns3Label {
    rotation: i32,
    style: String,
    text: String,
    x: i32,
    y: i32,
}

#[derive(Serialize)]
struct Gns3Drawing {
    drawing_id: String,
    x: i32,
    y: i32,
    z: u32,
    rotation: i32,
    svg: String,
}
