use std::sync::Arc;

use actix_web::{web, HttpRequest, HttpResponse};
use parking_lot::RwLock;

use crate::engine::simulation::SimulationEngine;

type AppState = web::Data<Arc<RwLock<SimulationEngine>>>;

pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: AppState,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let engine = state.get_ref().clone();
    let mut rx = engine.read().subscribe();

    // Spawn a task to forward simulation updates to the WebSocket client
    actix_rt::spawn(async move {
        loop {
            tokio::select! {
                // Forward tick updates to WS client
                result = rx.recv() => {
                    match result {
                        Ok(update) => {
                            if let Ok(json) = serde_json::to_string(&update) {
                                if session.text(json).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Skip missed updates
                            continue;
                        }
                        Err(_) => break,
                    }
                }
                // Handle incoming WS messages (ping/pong, close)
                msg = msg_stream.recv() => {
                    match msg {
                        Some(Ok(actix_ws::Message::Ping(bytes))) => {
                            let _ = session.pong(&bytes).await;
                        }
                        Some(Ok(actix_ws::Message::Close(_))) | None => break,
                        _ => {}
                    }
                }
            }
        }
    });

    Ok(response)
}
