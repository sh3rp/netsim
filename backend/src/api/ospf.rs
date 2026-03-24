use std::sync::Arc;

use actix_web::{web, HttpResponse};
use parking_lot::RwLock;

use crate::api::schemas::MessageResponse;
use crate::engine::simulation::SimulationEngine;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/ospf")
            .route("/{router_id}/lsdb", web::get().to(get_lsdb))
            .route("/{router_id}/neighbors", web::get().to(get_neighbors))
            .route("/{router_id}/routes", web::get().to(get_routes)),
    );
}

async fn get_lsdb(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let router_id = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => match &router.ospf_process {
            Some(ospf) => HttpResponse::Ok().json(&ospf.lsdb),
            None => HttpResponse::Ok().json(serde_json::json!({})),
        },
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}

async fn get_neighbors(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let router_id = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => match &router.ospf_process {
            Some(ospf) => HttpResponse::Ok().json(&ospf.neighbors),
            None => HttpResponse::Ok().json(serde_json::json!({})),
        },
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}

async fn get_routes(state: AppState, path: web::Path<String>) -> HttpResponse {
    let engine = state.read();
    let router_id = path.into_inner();
    match engine.topology.get_router(&router_id) {
        Some(router) => {
            // Filter RIB for OSPF routes only
            let ospf_routes: std::collections::HashMap<_, _> = router
                .rib
                .iter()
                .filter(|(_, e)| e.protocol == crate::engine::models::RouteProtocol::Ospf)
                .collect();
            HttpResponse::Ok().json(ospf_routes)
        }
        None => HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", router_id),
        }),
    }
}
