use super::state::AppState;
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Serialize)]
pub struct Metadata {
    pub api_version: u32,
    pub time_zone: String,
    pub timestamp_format: &'static str,
    pub schedule_time_basis: &'static str,
    pub event_stream: EventStreamMetadata,
}

#[derive(Serialize)]
pub struct EventStreamMetadata {
    pub path: &'static str,
    pub replay: bool,
    pub scope: &'static str,
}

pub async fn metadata(State(state): State<AppState>) -> Json<Metadata> {
    Json(Metadata {
        api_version: 1,
        time_zone: state.time_zone.to_string(),
        timestamp_format: "YYYY-MM-DD HH:mm:ss",
        schedule_time_basis: "hub_local",
        event_stream: EventStreamMetadata {
            path: "/events/stream",
            replay: false,
            scope: "instance",
        },
    })
}
