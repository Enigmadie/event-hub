use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use super::state::AppState;
use crate::domain::Device;

#[derive(Serialize)]
pub struct DeviceResponse {
    id: String,
    name: String,
    state: String,
    availability: String,
}

impl From<Device> for DeviceResponse {
    fn from(device: Device) -> Self {
        Self {
            id: device.id().as_str().to_string(),
            name: device.name().as_str().to_string(),
            state: format!("{:?}", device.status()),
            availability: format!("{:?}", device.availability()),
        }
    }
}

pub async fn list_devices(State(state): State<AppState>) -> Json<Vec<DeviceResponse>> {
    let devices = state
        .app_service
        .list_devices()
        .into_iter()
        .map(DeviceResponse::from)
        .collect();

    Json(devices)
}

pub async fn turn_on(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.turn_on(&id))
}

pub async fn turn_off(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.turn_off(&id))
}

fn command_status(result: anyhow::Result<()>) -> StatusCode {
    match result {
        Ok(()) => StatusCode::ACCEPTED,
        Err(error) => {
            log::error!("device command failed: {error:#}");
            StatusCode::BAD_GATEWAY
        }
    }
}
