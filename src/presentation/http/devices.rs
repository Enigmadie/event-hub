use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use super::{errors::ApiError, state::AppState};
use crate::application::{
    app_service::DeviceSummary,
    device_event::DeviceEventLogEntry,
    recurring_command::{DeviceCommand, RecurringCommand},
    recurring_schedule::RecurringSchedule,
    scheduled_command::{ScheduledCommand, ScheduledCommandJob, ScheduledCommandStatus},
};

#[derive(Serialize)]
pub struct DeviceResponse {
    id: String,
    name: String,
    availability: String,
    supported_commands: Option<Vec<DeviceCommand>>,
    values: serde_json::Map<String, serde_json::Value>,
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
    values: Option<serde_json::Map<String, serde_json::Value>>,
    source_topic: String,
    payload: serde_json::Value,
    occurred_at: String,
}

#[derive(Deserialize)]
pub struct CreateScheduleRequest {
    command: String,
    run_at: String,
}

#[derive(Deserialize)]
pub struct CreateRecurringScheduleRequest {
    start_time: String,
    end_time: String,
}

#[derive(Deserialize)]
pub struct UpdateRecurringScheduleRequest {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct CreateRecurringCommandRequest {
    command: String,
    #[serde(default = "empty_payload")]
    payload: serde_json::Value,
    local_time: String,
}

#[derive(Deserialize)]
pub struct UpdateRecurringCommandRequest {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct SetCoverPositionRequest {
    position: u8,
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

#[derive(Serialize)]
pub struct RecurringScheduleResponse {
    id: i64,
    device_id: String,
    start_time: String,
    end_time: String,
    enabled: bool,
    last_started_on: Option<String>,
    last_ended_on: Option<String>,
    last_error: Option<String>,
}

#[derive(Serialize)]
pub struct RecurringCommandResponse {
    id: i64,
    device_id: String,
    command: String,
    payload: serde_json::Value,
    local_time: String,
    enabled: bool,
    last_run_on: Option<String>,
    last_error: Option<String>,
}

impl From<DeviceSummary> for DeviceResponse {
    fn from(summary: DeviceSummary) -> Self {
        let device = summary.device;
        Self {
            id: device.id().as_str().to_string(),
            name: device.name().as_str().to_string(),
            availability: format!("{:?}", device.availability()),
            values: summary.latest_values,
            supported_commands: summary.supported_commands,
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

impl From<RecurringSchedule> for RecurringScheduleResponse {
    fn from(schedule: RecurringSchedule) -> Self {
        Self {
            id: schedule.id,
            device_id: schedule.device_id.as_str().to_string(),
            start_time: schedule.start_time,
            end_time: schedule.end_time,
            enabled: schedule.enabled,
            last_started_on: schedule.last_started_on,
            last_ended_on: schedule.last_ended_on,
            last_error: schedule.last_error,
        }
    }
}

impl From<RecurringCommand> for RecurringCommandResponse {
    fn from(command: RecurringCommand) -> Self {
        Self {
            id: command.id,
            device_id: command.device_id.as_str().to_string(),
            command: device_command_to_api(command.command).to_string(),
            payload: command.payload,
            local_time: command.local_time,
            enabled: command.enabled,
            last_run_on: command.last_run_on,
            last_error: command.last_error,
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
            values: event.values,
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
        return ApiError::invalid(
            "invalid_command",
            "The command is not supported by this endpoint.",
            "command",
        )
        .into_response();
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

pub async fn list_recurring_schedules(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let schedules: Vec<RecurringScheduleResponse> =
        match state.app_service.list_recurring_schedules(&id).await {
            Ok(schedules) => schedules
                .into_iter()
                .map(RecurringScheduleResponse::from)
                .collect(),
            Err(error) => {
                log::error!("failed to list recurring schedules: {error:#}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    Json(schedules).into_response()
}

pub async fn create_recurring_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<CreateRecurringScheduleRequest>,
) -> impl IntoResponse {
    match state
        .app_service
        .create_recurring_schedule(&id, request.start_time, request.end_time)
        .await
    {
        Ok(schedule) => (
            StatusCode::CREATED,
            Json(RecurringScheduleResponse::from(schedule)),
        )
            .into_response(),
        Err(error) => {
            log::error!("failed to create recurring schedule: {error:#}");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

pub async fn update_recurring_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateRecurringScheduleRequest>,
) -> StatusCode {
    match state
        .app_service
        .set_recurring_schedule_enabled(id, request.enabled)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(error) => {
            log::error!("failed to update recurring schedule: {error:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn list_recurring_commands(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let commands: Vec<RecurringCommandResponse> =
        match state.app_service.list_recurring_commands(&id).await {
            Ok(commands) => commands
                .into_iter()
                .map(RecurringCommandResponse::from)
                .collect(),
            Err(error) => {
                log::error!("failed to list recurring commands: {error:#}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    Json(commands).into_response()
}

pub async fn create_recurring_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<CreateRecurringCommandRequest>,
) -> impl IntoResponse {
    let Some(command) = device_command_from_api(&request.command) else {
        return ApiError::invalid(
            "invalid_command",
            "The command is not supported by this endpoint.",
            "command",
        )
        .into_response();
    };

    if command == DeviceCommand::SetPosition && !valid_position_payload(&request.payload) {
        return ApiError::invalid(
            "invalid_position",
            "Position must be an integer from 0 to 100.",
            "payload.position",
        )
        .into_response();
    }

    match state
        .app_service
        .create_recurring_command(&id, command, request.payload, request.local_time)
        .await
    {
        Ok(command) => (
            StatusCode::CREATED,
            Json(RecurringCommandResponse::from(command)),
        )
            .into_response(),
        Err(error) => {
            log::error!("failed to create recurring command: {error:#}");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

pub async fn update_recurring_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateRecurringCommandRequest>,
) -> StatusCode {
    match state
        .app_service
        .set_recurring_command_enabled(id, request.enabled)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(error) => {
            log::error!("failed to update recurring command: {error:#}");
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

pub async fn open_cover(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.open_cover(&id))
}

pub async fn close_cover(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.close_cover(&id))
}

pub async fn stop_cover(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    command_status(state.app_service.stop_cover(&id))
}

pub async fn set_cover_position(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<SetCoverPositionRequest>,
) -> axum::response::Response {
    if request.position > 100 {
        return ApiError::invalid(
            "invalid_position",
            "Position must be an integer from 0 to 100.",
            "position",
        )
        .into_response();
    }

    command_status(state.app_service.set_cover_position(&id, request.position)).into_response()
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

fn device_command_from_api(value: &str) -> Option<DeviceCommand> {
    match value {
        "turn_on" => Some(DeviceCommand::TurnOn),
        "turn_off" => Some(DeviceCommand::TurnOff),
        "open" => Some(DeviceCommand::Open),
        "close" => Some(DeviceCommand::Close),
        "stop" => Some(DeviceCommand::Stop),
        "set_position" => Some(DeviceCommand::SetPosition),
        _ => None,
    }
}

fn device_command_to_api(command: DeviceCommand) -> &'static str {
    match command {
        DeviceCommand::TurnOn => "turn_on",
        DeviceCommand::TurnOff => "turn_off",
        DeviceCommand::Open => "open",
        DeviceCommand::Close => "close",
        DeviceCommand::Stop => "stop",
        DeviceCommand::SetPosition => "set_position",
    }
}

fn valid_position_payload(payload: &serde_json::Value) -> bool {
    payload
        .get("position")
        .and_then(|value| value.as_u64())
        .is_some_and(|position| position <= 100)
}

fn empty_payload() -> serde_json::Value {
    serde_json::json!({})
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
