use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use super::state::AppState;
use crate::{
    application::{
        device_event::DeviceEventLogEntry,
        scheduled_command::{ScheduledCommand, ScheduledCommandJob, ScheduledCommandStatus},
    },
    domain::Device,
};

#[derive(Serialize)]
pub struct DeviceResponse {
    id: String,
    name: String,
    state: String,
    availability: String,
}

#[derive(Deserialize)]
pub struct DeviceEventsQuery {
    limit: Option<i64>,
}

#[derive(Serialize)]
pub struct DeviceEventResponse {
    id: i64,
    device_id: String,
    kind: String,
    name: Option<String>,
    state: Option<String>,
    availability: Option<String>,
    source_topic: String,
    payload: serde_json::Value,
    occurred_at: String,
}

#[derive(Deserialize)]
pub struct CreateScheduleRequest {
    command: String,
    run_at: String,
}

#[derive(Serialize)]
pub struct ScheduledCommandResponse {
    id: i64,
    device_id: String,
    command: String,
    status: String,
    run_at: String,
    last_error: Option<String>,
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

impl From<ScheduledCommandJob> for ScheduledCommandResponse {
    fn from(job: ScheduledCommandJob) -> Self {
        Self {
            id: job.id,
            device_id: job.device_id.as_str().to_string(),
            command: command_to_api(job.command).to_string(),
            status: status_to_api(job.status).to_string(),
            run_at: job.run_at,
            last_error: job.last_error,
        }
    }
}

impl From<DeviceEventLogEntry> for DeviceEventResponse {
    fn from(event: DeviceEventLogEntry) -> Self {
        Self {
            id: event.id,
            device_id: event.device_id.as_str().to_string(),
            kind: format!("{:?}", event.kind),
            name: event.name,
            state: event.state.map(|state| format!("{state:?}")),
            availability: event
                .availability
                .map(|availability| format!("{availability:?}")),
            source_topic: event.source_topic,
            payload: event.payload,
            occurred_at: event.occurred_at,
        }
    }
}

pub async fn list_devices(State(state): State<AppState>) -> impl IntoResponse {
    let devices: Vec<DeviceResponse> = match state.app_service.list_devices().await {
        Ok(devices) => devices.into_iter().map(DeviceResponse::from).collect(),
        Err(error) => {
            log::error!("failed to list devices: {error:#}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(devices).into_response()
}

pub async fn list_device_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<DeviceEventsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let events: Vec<DeviceEventResponse> =
        match state.app_service.list_device_events(&id, limit).await {
            Ok(events) => events.into_iter().map(DeviceEventResponse::from).collect(),
            Err(error) => {
                log::error!("failed to list device events: {error:#}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    Json(events).into_response()
}

pub async fn list_schedules(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let jobs: Vec<ScheduledCommandResponse> =
        match state.app_service.list_scheduled_commands(&id).await {
            Ok(jobs) => jobs
                .into_iter()
                .map(ScheduledCommandResponse::from)
                .collect(),
            Err(error) => {
                log::error!("failed to list scheduled commands: {error:#}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    Json(jobs).into_response()
}

pub async fn create_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<CreateScheduleRequest>,
) -> impl IntoResponse {
    let Some(command) = command_from_api(&request.command) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match state
        .app_service
        .schedule_command(&id, command, request.run_at)
        .await
    {
        Ok(job) => (
            StatusCode::CREATED,
            Json(ScheduledCommandResponse::from(job)),
        )
            .into_response(),
        Err(error) => {
            log::error!("failed to create scheduled command: {error:#}");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

pub async fn cancel_schedule(State(state): State<AppState>, Path(id): Path<i64>) -> StatusCode {
    match state.app_service.cancel_scheduled_command(id).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(error) => {
            log::error!("failed to cancel scheduled command: {error:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn turn_on(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.turn_on(&id))
}

pub async fn turn_off(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.turn_off(&id))
}

fn command_from_api(value: &str) -> Option<ScheduledCommand> {
    match value {
        "turn_on" => Some(ScheduledCommand::TurnOn),
        "turn_off" => Some(ScheduledCommand::TurnOff),
        _ => None,
    }
}

fn command_to_api(command: ScheduledCommand) -> &'static str {
    match command {
        ScheduledCommand::TurnOn => "turn_on",
        ScheduledCommand::TurnOff => "turn_off",
    }
}

fn status_to_api(status: ScheduledCommandStatus) -> &'static str {
    match status {
        ScheduledCommandStatus::Pending => "pending",
        ScheduledCommandStatus::Running => "running",
        ScheduledCommandStatus::Succeeded => "succeeded",
        ScheduledCommandStatus::Failed => "failed",
        ScheduledCommandStatus::Cancelled => "cancelled",
    }
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
