use std::sync::Arc;

use actix_web::{web, HttpResponse};
use parking_lot::RwLock;

use crate::api::schemas::*;
use crate::engine::models::Topology;
use crate::engine::simulation::SimulationEngine;
use crate::persistence::store;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/simulation")
            .route("/state", web::get().to(get_state))
            .route("/start", web::post().to(start))
            .route("/stop", web::post().to(stop))
            .route("/step", web::post().to(step))
            .route("/tick-rate", web::put().to(set_tick_rate))
            .route("/save", web::post().to(save))
            .route("/load", web::post().to(load))
            .route("/load-sample", web::post().to(load_sample))
            .route("/reset", web::post().to(reset)),
    );
}

async fn get_state(state: AppState) -> HttpResponse {
    let engine = state.read();
    HttpResponse::Ok().json(&engine.state)
}

async fn start(state: AppState) -> HttpResponse {
    let mut engine = state.write();
    engine.state.running = true;
    HttpResponse::Ok().json(MessageResponse {
        message: "Simulation started".to_string(),
    })
}

async fn stop(state: AppState) -> HttpResponse {
    let mut engine = state.write();
    engine.state.running = false;
    HttpResponse::Ok().json(MessageResponse {
        message: "Simulation stopped".to_string(),
    })
}

async fn step(state: AppState) -> HttpResponse {
    let mut engine = state.write();
    let update = engine.step();
    HttpResponse::Ok().json(update)
}

async fn set_tick_rate(state: AppState, body: web::Json<TickRateRequest>) -> HttpResponse {
    let mut engine = state.write();
    engine.state.tick_rate_ms = body.tick_rate_ms.max(10).min(5000);
    HttpResponse::Ok().json(MessageResponse {
        message: format!("Tick rate set to {}ms", engine.state.tick_rate_ms),
    })
}

async fn save(state: AppState) -> HttpResponse {
    let engine = state.read();
    match store::save_topology(&engine.topology) {
        Ok(json) => HttpResponse::Ok()
            .content_type("application/json")
            .body(json),
        Err(e) => HttpResponse::InternalServerError().json(MessageResponse {
            message: format!("Failed to save: {}", e),
        }),
    }
}

async fn load(state: AppState, body: web::Bytes) -> HttpResponse {
    let json = String::from_utf8_lossy(&body);
    match store::load_topology(&json) {
        Ok(topology) => {
            let mut engine = state.write();
            engine.topology = topology;
            engine.state.tick = 0;
            engine.state.running = false;
            HttpResponse::Ok().json(MessageResponse {
                message: "Topology loaded".to_string(),
            })
        }
        Err(e) => HttpResponse::BadRequest().json(MessageResponse {
            message: format!("Failed to load: {}", e),
        }),
    }
}

async fn reset(state: AppState) -> HttpResponse {
    let mut engine = state.write();
    engine.topology = Topology::default();
    engine.state.tick = 0;
    engine.state.running = false;
    HttpResponse::Ok().json(MessageResponse {
        message: "Simulation reset".to_string(),
    })
}

async fn load_sample(state: AppState) -> HttpResponse {
    let mut engine = state.write();
    engine.topology = store::create_sample_topology();
    engine.state.tick = 0;
    engine.state.running = false;
    HttpResponse::Ok().json(MessageResponse {
        message: "Sample topology loaded".to_string(),
    })
}
