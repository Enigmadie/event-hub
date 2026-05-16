use axum::{
    Router,
    routing::{get, patch, post},
};
use tower_http::cors::CorsLayer;

use super::{devices, handlers::health, state::AppState};

pub fn create_router(state: AppState, cors: CorsLayer) -> Router {
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
            "/devices/:id/recurring-commands",
            get(devices::list_recurring_commands).post(devices::create_recurring_command),
        )
        .route(
            "/schedules/:id",
            axum::routing::delete(devices::cancel_schedule),
        )
        .route(
            "/recurring-schedules/:id",
            patch(devices::update_recurring_schedule),
        )
        .route(
            "/recurring-commands/:id",
            patch(devices::update_recurring_command),
        )
        .route("/devices/:id/turn-on", post(devices::turn_on))
        .route("/devices/:id/turn-off", post(devices::turn_off))
        .route("/devices/:id/open", post(devices::open_cover))
        .route("/devices/:id/close", post(devices::close_cover))
        .route("/devices/:id/stop", post(devices::stop_cover))
        .route("/devices/:id/position", post(devices::set_cover_position))
        .with_state(state)
        .layer(cors)
}
