use std::sync::Arc;

use actix_web::{web, HttpResponse};
use parking_lot::RwLock;

use crate::api::schemas::*;
use crate::engine::simulation::SimulationEngine;
use crate::persistence::store;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/bgp")
            .route("/{router_id}/neighbors", web::get().to(get_neighbors))
            .route("/{router_id}/neighbors", web::post().to(add_neighbor))
            .route("/{router_id}/routes", web::get().to(get_routes))
            .route("/{router_id}/enable", web::post().to(enable_bgp))
            .route(
                "/{router_id}/adj-rib-in/{neighbor}",
                web::get().to(get_adj_rib_in),
            ),
    );
}

async fn get_neighbors(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let router_id = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => match &router.bgp_process {
            Some(bgp) => HttpResponse::Ok().json(&bgp.neighbors),
            None => HttpResponse::Ok().json(serde_json::json!({})),
        },
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}

async fn add_neighbor(
    state: AppState,
    path: web::Path<String>,
    body: web::Json<AddBgpNeighborRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let router_id = path.into_inner();

    let neighbor_ip: std::net::Ipv4Addr = match body.neighbor_ip.parse() {
        Ok(ip) => ip,
        Err(_) => {
            return HttpResponse::BadRequest().json(MessageResponse {
                message: "Invalid neighbor IP".to_string(),
            })
        }
    };

    if let Some(router) = engine.topology.get_router_mut(&router_id) {
        let local_asn = router
            .bgp_process
            .as_ref()
            .map(|b| b.local_asn)
            .unwrap_or(0);
        store::add_bgp_neighbor(
            router,
            neighbor_ip,
            body.remote_asn,
            local_asn,
            body.import_policy.clone(),
            body.export_policy.clone(),
        );
        HttpResponse::Created().json(MessageResponse {
            message: format!("Added BGP neighbor {}", body.neighbor_ip),
        })
    } else {
        HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        })
    }
}

async fn get_routes(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let router_id = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => match &router.bgp_process {
            Some(bgp) => HttpResponse::Ok().json(&bgp.loc_rib),
            None => HttpResponse::Ok().json(serde_json::json!({})),
        },
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}

async fn enable_bgp(
    state: AppState,
    path: web::Path<String>,
    body: web::Json<EnableBgpRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let router_id = path.into_inner();

    if let Some(router) = engine.topology.get_router_mut(&router_id) {
        store::enable_bgp(router, body.local_asn);
        HttpResponse::Ok().json(MessageResponse {
            message: format!("BGP enabled on {}", router_id),
        })
    } else {
        HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        })
    }
}

async fn get_adj_rib_in(
    state: AppState,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let engine = state.read();
    let (router_id, neighbor_key) = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => match &router.bgp_process {
            Some(bgp) => match bgp.neighbors.get(&neighbor_key) {
                Some(neighbor) => HttpResponse::Ok().json(&neighbor.adj_rib_in),
                None => HttpResponse::NotFound().json(MessageResponse {
                    message: format!("Neighbor {} not found", neighbor_key),
                }),
            },
            None => HttpResponse::Ok().json(serde_json::json!({})),
        },
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}
