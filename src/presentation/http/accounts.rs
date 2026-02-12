use axum::{extract::State, http::StatusCode, Json};
use serde_json::Value;

use super::state::AppState;

pub async fn get_account_info(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match state.account_service.get_account_info().await {
        Ok(info) => {
            let v = serde_json::to_value(info).unwrap();
            (StatusCode::OK, Json(v))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Request failed: {}", e) })),
        ),
    }
}
