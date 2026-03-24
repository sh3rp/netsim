use std::sync::Arc;

use actix_web::{web, HttpResponse};
use parking_lot::RwLock;

use crate::api::schemas::*;
use crate::engine::models::TrafficGenerator;
use crate::engine::simulation::SimulationEngine;
use crate::engine::traffic;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/traffic")
            .route("/generators", web::get().to(list_generators))
            .route("/generators", web::post().to(create_generator))
            .route("/generators/{gen_id}", web::put().to(update_generator))
            .route("/generators/{gen_id}", web::delete().to(delete_generator))
            .route("/flows", web::get().to(get_flows))
            .route("/link-utilization", web::get().to(get_link_utilization)),
    );
}

async fn list_generators(state: AppState) -> HttpResponse {
    let engine = state.read();
    let generators: Vec<&TrafficGenerator> = engine
        .topology
        .all_routers()
        .iter()
        .flat_map(|r| r.traffic_generators.iter())
        .collect();
    HttpResponse::Ok().json(generators)
}

async fn create_generator(
    state: AppState,
    body: web::Json<CreateTrafficGeneratorRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let gen_id = format!("tg-{}", uuid::Uuid::new_v4());

    let gen = TrafficGenerator {
        id: gen_id.clone(),
        source_router_id: body.source_router_id.clone(),
        dest_prefix: body.dest_prefix.clone(),
        rate_bps: body.rate_bps,
        is_active: body.is_active,
    };

    if let Some(router) = engine.topology.get_router_mut(&body.source_router_id) {
        router.traffic_generators.push(gen);
        HttpResponse::Created().json(IdResponse { id: gen_id })
    } else {
        HttpResponse::NotFound().json(MessageResponse {
            message: format!("Router {} not found", body.source_router_id),
        })
    }
}

async fn update_generator(
    state: AppState,
    path: web::Path<String>,
    body: web::Json<UpdateTrafficGeneratorRequest>,
) -> HttpResponse {
    let mut engine = state.write();
    let gen_id = path.into_inner();

    for router in engine.topology.all_routers_mut() {
        for gen in &mut router.traffic_generators {
            if gen.id == gen_id {
                if let Some(rate) = body.rate_bps {
                    gen.rate_bps = rate;
                }
                if let Some(active) = body.is_active {
                    gen.is_active = active;
                }
                if let Some(ref prefix) = body.dest_prefix {
                    gen.dest_prefix = prefix.clone();
                }
                return HttpResponse::Ok().json(MessageResponse {
                    message: format!("Updated generator {}", gen_id),
                });
            }
        }
    }

    HttpResponse::NotFound().json(MessageResponse {
        message: format!("Generator {} not found", gen_id),
    })
}

async fn delete_generator(state: AppState, path: web::Path<String>) -> HttpResponse {
    let mut engine = state.write();
    let gen_id = path.into_inner();

    for router in engine.topology.all_routers_mut() {
        let before = router.traffic_generators.len();
        router.traffic_generators.retain(|g| g.id != gen_id);
        if router.traffic_generators.len() < before {
            return HttpResponse::Ok().json(MessageResponse {
                message: format!("Deleted generator {}", gen_id),
            });
        }
    }

    HttpResponse::NotFound().json(MessageResponse {
        message: format!("Generator {} not found", gen_id),
    })
}

async fn get_flows(state: AppState) -> HttpResponse {
    let engine = state.read();
    let flows = traffic::compute_flows(&engine.topology);
    HttpResponse::Ok().json(flows)
}

async fn get_link_utilization(state: AppState) -> HttpResponse {
    let engine = state.read();
    let utilization = traffic::get_link_utilization(&engine.topology);
    HttpResponse::Ok().json(utilization)
}
