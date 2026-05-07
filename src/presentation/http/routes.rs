use axum::{
    Router,
    routing::{get, post},
};

use super::{devices, handlers::health, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/devices", get(devices::list_devices))
        .route("/devices/:id/turn-on", post(devices::turn_on))
        .route("/devices/:id/turn-off", post(devices::turn_off))
        .with_state(state)
}
