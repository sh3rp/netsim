use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use parking_lot::RwLock;

mod api;
mod engine;
mod persistence;

use engine::simulation::{run_simulation_loop, SimulationEngine};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    log::info!("Starting NetSim server on http://0.0.0.0:8080");

    let engine = Arc::new(RwLock::new(SimulationEngine::new()));

    // Start the simulation loop in a background task
    let sim_engine = engine.clone();
    tokio::spawn(async move {
        run_simulation_loop(sim_engine).await;
    });

    let app_state = web::Data::new(engine);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .service(
                web::scope("/api/v1")
                    .configure(api::topology::config)
                    .configure(api::ospf::config)
                    .configure(api::bgp::config)
                    .configure(api::traffic::config)
                    .configure(api::simulation::config)
                    .route("/policies", web::get().to(list_policies))
                    .route("/policies", web::post().to(create_policy))
                    .route("/policies/{name}", web::delete().to(delete_policy)),
            )
            .route("/ws", web::get().to(api::websocket::ws_handler))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

async fn list_policies(
    state: web::Data<Arc<RwLock<SimulationEngine>>>,
) -> actix_web::HttpResponse {
    let engine = state.read();
    actix_web::HttpResponse::Ok().json(&engine.topology.policies)
}

async fn create_policy(
    state: web::Data<Arc<RwLock<SimulationEngine>>>,
    body: web::Json<api::schemas::PolicyRequest>,
) -> actix_web::HttpResponse {
    match engine::policies::parse_policy(&body.policy_text) {
        Ok(policy) => {
            let name = policy.name.clone();
            let mut engine = state.write();
            engine.topology.policies.insert(name.clone(), policy);
            actix_web::HttpResponse::Created().json(api::schemas::IdResponse { id: name })
        }
        Err(e) => actix_web::HttpResponse::BadRequest().json(api::schemas::MessageResponse {
            message: format!("Policy parse error: {}", e),
        }),
    }
}

async fn delete_policy(
    state: web::Data<Arc<RwLock<SimulationEngine>>>,
    path: web::Path<String>,
) -> actix_web::HttpResponse {
    let mut engine = state.write();
    let name = path.into_inner();
    match engine.topology.policies.remove(&name) {
        Some(_) => actix_web::HttpResponse::Ok().json(api::schemas::MessageResponse {
            message: format!("Deleted policy {}", name),
        }),
        None => actix_web::HttpResponse::NotFound().json(api::schemas::MessageResponse {
            message: format!("Policy {} not found", name),
        }),
    }
}
