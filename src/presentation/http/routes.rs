use axum::{
    Router,
    routing::{get, patch, post},
};

use super::{devices, handlers::health, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/devices", get(devices::list_devices))
        .route("/devices/:id/events", get(devices::list_device_events))
        .route(
            "/devices/:id/schedules",
            get(devices::list_schedules).post(devices::create_schedule),
        )
        .route(
            "/devices/:id/recurring-schedules",
            get(devices::list_recurring_schedules).post(devices::create_recurring_schedule),
        )
        .route(
            "/schedules/:id",
            axum::routing::delete(devices::cancel_schedule),
        )
        .route(
            "/recurring-schedules/:id",
            patch(devices::update_recurring_schedule),
        )
        .route("/devices/:id/turn-on", post(devices::turn_on))
        .route("/devices/:id/turn-off", post(devices::turn_off))
        .with_state(state)
}
