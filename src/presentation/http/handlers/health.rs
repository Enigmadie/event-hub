use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::presentation::http::state::AppState;

pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let mqtt_connected = state.mqtt_health.is_connected();
    if mqtt_connected {
        (
            StatusCode::OK,
            Json(json!({ "status": "ok", "mqtt": "connected" })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "degraded", "mqtt": "disconnected" })),
        )
    }
}
