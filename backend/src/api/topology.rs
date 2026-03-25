use std::collections::HashSet;
use std::sync::Arc;

use actix_web::{web, HttpResponse};
use parking_lot::RwLock;

use crate::api::schemas::*;
use crate::engine::simulation::SimulationEngine;
use crate::persistence::store;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/topology")
            .route("/as", web::get().to(list_autonomous_systems))
            .route("/as", web::post().to(create_autonomous_system))
            .route("/as/{as_id}", web::get().to(get_autonomous_system))
            .route("/as/{as_id}", web::delete().to(delete_autonomous_system))
            .route("/as/{as_id}/routers", web::get().to(list_routers))
            .route("/as/{as_id}/routers", web::post().to(create_router))
            .route("/routers", web::get().to(list_standalone_routers))
            .route("/routers", web::post().to(create_standalone_router))
            .route("/routers/{router_id}", web::get().to(get_router))
            .route("/routers/{router_id}", web::delete().to(delete_router))
            .route(
                "/routers/{router_id}/interfaces",
                web::post().to(add_interface),
            )
            .route("/links", web::get().to(list_links))
            .route("/links", web::post().to(create_link))
            .route("/links/{link_id}", web::delete().to(delete_link))
            .route("/links/{link_id}/state", web::put().to(set_link_state))
            .route("/snapshot", web::get().to(get_topology_snapshot)),
    );
}

async fn list_autonomous_systems(state: AppState) -> HttpResponse {
    let engine = state.read();
    let systems: Vec<_> = engine.topology.autonomous_systems.values().collect();
    HttpResponse::Ok().json(systems)
}

async fn create_autonomous_system(
    state: AppState,
    body: web::Json<CreateAsRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let asys = store::create_autonomous_system(body.asn, &body.name);
    let id = asys.id.clone();
    engine.topology.autonomous_systems.insert(id.clone(), asys);
    HttpResponse::Created().json(IdResponse { id })
}

async fn get_autonomous_system(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let as_id = path.into_inner();
    match engine.topology.autonomous_systems.get(&as_id) {
        Some(asys) => HttpResponse::Ok().json(asys),
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("AS {} not found", as_id),
        }),
    }
}

async fn delete_autonomous_system(state: AppState, path: web::Path<String>) -> HttpResponse {
    let mut engine = state.write();
    let as_id = path.into_inner();
    match engine.topology.autonomous_systems.remove(&as_id) {
        Some(asys) => {
            // Remove all links connected to interfaces that belonged to this AS.
            let iface_ids: HashSet<String> = asys
                .routers
                .values()
                .flat_map(|r| r.interfaces.keys().cloned())
                .collect();
            let links_to_remove = collect_links_for_interfaces(&engine.topology, &iface_ids);
            for link_id in links_to_remove {
                remove_link_and_detach_interfaces(&mut engine.topology, &link_id);
            }

            HttpResponse::Ok().json(MessageResponse {
                message: format!("Deleted AS {}", as_id),
            })
        }
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("AS {} not found", as_id),
        }),
    }
}

async fn list_routers(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let as_id = path.into_inner();
    match engine.topology.autonomous_systems.get(&as_id) {
        Some(asys) => {
            let routers: Vec<_> = asys.routers.values().collect();
            HttpResponse::Ok().json(routers)
        }
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("AS {} not found", as_id),
        }),
    }
}

async fn create_router(
    state: AppState,
    path: web::Path<String>,
    body: web::Json<CreateRouterRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let as_id = path.into_inner();

    let router_ip: std::net::Ipv4Addr = match body.router_id_ip.parse() {
        Ok(ip) => ip,
        Err(_) => {
            return HttpResponse::BadRequest().json(MessageResponse {
                message: "Invalid router ID IP".to_string(),
            })
        }
    };

    let router = store::create_router(
        &body.name,
        &as_id,
        router_ip,
        (body.position_x, body.position_y),
    );
    let id = router.id.clone();

    match engine.topology.autonomous_systems.get_mut(&as_id) {
        Some(asys) => {
            asys.routers.insert(id.clone(), router);
            HttpResponse::Created().json(IdResponse { id })
        }
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("AS {} not found", as_id),
        }),
    }
}

async fn list_standalone_routers(state: AppState) -> HttpResponse {
    let engine = state.read();
    let routers: Vec<_> = engine.topology.standalone_routers.values().collect();
    HttpResponse::Ok().json(routers)
}

async fn create_standalone_router(
    state: AppState,
    body: web::Json<CreateRouterRequest>,
) -> HttpResponse {
    let mut engine = state.write();

    let router_ip: std::net::Ipv4Addr = match body.router_id_ip.parse() {
        Ok(ip) => ip,
        Err(_) => {
            return HttpResponse::BadRequest().json(MessageResponse {
                message: "Invalid router ID IP".to_string(),
            })
        }
    };

    let router = store::create_router(
        &body.name,
        "standalone",
        router_ip,
        (body.position_x, body.position_y),
    );
    let id = router.id.clone();
    engine
        .topology
        .standalone_routers
        .insert(id.clone(), router);
    HttpResponse::Created().json(IdResponse { id })
}

async fn get_router(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let router_id = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => HttpResponse::Ok().json(router),
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}

async fn delete_router(state: AppState, path: web::Path<String>) -> HttpResponse {
    let mut engine = state.write();
    let router_id = path.into_inner();
    match engine.topology.remove_router(&router_id) {
        Some(router) => {
            // Remove all links connected to interfaces that belonged to this router.
            let iface_ids: HashSet<String> = router.interfaces.keys().cloned().collect();
            let links_to_remove = collect_links_for_interfaces(&engine.topology, &iface_ids);
            for link_id in links_to_remove {
                remove_link_and_detach_interfaces(&mut engine.topology, &link_id);
            }

            HttpResponse::Ok().json(MessageResponse {
                message: format!("Deleted router {}", router_id),
            })
        }
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}

async fn add_interface(
    state: AppState,
    path: web::Path<String>,
    body: web::Json<AddInterfaceRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let router_id = path.into_inner();

    let ip: ipnetwork::Ipv4Network = match body.ip_address.parse() {
        Ok(ip) => ip,
        Err(_) => {
            return HttpResponse::BadRequest().json(MessageResponse {
                message: "Invalid IP address".to_string(),
            })
        }
    };

    if let Some(router) = engine.topology.get_router_mut(&router_id) {
        let iface_id = store::add_interface(router, &body.name, ip, body.bandwidth, body.cost);
        HttpResponse::Created().json(IdResponse { id: iface_id })
    } else {
        HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        })
    }
}

async fn list_links(state: AppState) -> HttpResponse {
    let engine = state.read();
    let links: Vec<_> = engine.topology.links.values().collect();
    HttpResponse::Ok().json(links)
}

async fn create_link(state: AppState, body: web::Json<CreateLinkRequest>) -> HttpResponse {
    let mut engine = state.write();

    if body.interface_a_id == body.interface_b_id {
        return HttpResponse::BadRequest().json(MessageResponse {
            message: "Link endpoints must be different interfaces".to_string(),
        });
    }

    let iface_a = find_interface(&engine.topology, &body.interface_a_id);
    let iface_b = find_interface(&engine.topology, &body.interface_b_id);
    let Some(iface_a) = iface_a else {
        return HttpResponse::BadRequest().json(MessageResponse {
            message: format!("Interface {} not found", body.interface_a_id),
        });
    };
    let Some(iface_b) = iface_b else {
        return HttpResponse::BadRequest().json(MessageResponse {
            message: format!("Interface {} not found", body.interface_b_id),
        });
    };
    if iface_a.link_id.is_some() || iface_b.link_id.is_some() {
        return HttpResponse::BadRequest().json(MessageResponse {
            message: "One or both interfaces are already linked".to_string(),
        });
    }

    let link_id = store::create_link(
        &mut engine.topology,
        &body.interface_a_id,
        &body.interface_b_id,
        body.bandwidth,
        body.delay_ms,
    );
    HttpResponse::Created().json(IdResponse { id: link_id })
}

async fn delete_link(state: AppState, path: web::Path<String>) -> HttpResponse {
    let mut engine = state.write();
    let link_id = path.into_inner();
    match remove_link_and_detach_interfaces(&mut engine.topology, &link_id) {
        true => HttpResponse::Ok().json(MessageResponse {
            message: format!("Deleted link {}", link_id),
        }),
        false => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Link {} not found", link_id),
        }),
    }
}

async fn set_link_state(
    state: AppState,
    path: web::Path<String>,
    body: web::Json<LinkStateRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let link_id = path.into_inner();

    use crate::engine::events::EventType;
    let tick = engine.state.tick;
    engine.event_queue.schedule(
        tick + 1,
        5,
        EventType::LinkStateChange {
            link_id: link_id.clone(),
            is_up: body.is_up,
        },
    );

    HttpResponse::Ok().json(MessageResponse {
        message: format!("Link {} state change scheduled", link_id),
    })
}

async fn get_topology_snapshot(state: AppState) -> HttpResponse {
    let engine = state.read();
    HttpResponse::Ok().json(&engine.topology)
}

fn find_interface<'a>(
    topology: &'a crate::engine::models::Topology,
    iface_id: &str,
) -> Option<&'a crate::engine::models::Interface> {
    for router in topology.all_routers() {
        if let Some(iface) = router.interfaces.get(iface_id) {
            return Some(iface);
        }
    }
    None
}

fn collect_links_for_interfaces(
    topology: &crate::engine::models::Topology,
    iface_ids: &HashSet<String>,
) -> Vec<String> {
    topology
        .links
        .values()
        .filter(|link| {
            iface_ids.contains(&link.interface_a_id) || iface_ids.contains(&link.interface_b_id)
        })
        .map(|link| link.id.clone())
        .collect()
}

fn remove_link_and_detach_interfaces(
    topology: &mut crate::engine::models::Topology,
    link_id: &str,
) -> bool {
    let Some(link) = topology.links.get(link_id) else {
        return false;
    };
    let iface_a = link.interface_a_id.clone();
    let iface_b = link.interface_b_id.clone();

    for router in topology.standalone_routers.values_mut() {
        if let Some(iface) = router.interfaces.get_mut(&iface_a) {
            iface.link_id = None;
        }
        if let Some(iface) = router.interfaces.get_mut(&iface_b) {
            iface.link_id = None;
        }
    }
    for asys in topology.autonomous_systems.values_mut() {
        for router in asys.routers.values_mut() {
            if let Some(iface) = router.interfaces.get_mut(&iface_a) {
                iface.link_id = None;
            }
            if let Some(iface) = router.interfaces.get_mut(&iface_b) {
                iface.link_id = None;
            }
        }
    }
    topology.links.remove(link_id);
    true
}
