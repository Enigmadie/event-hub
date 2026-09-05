mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use event_hub::{
    application::changes::{ChangeKind, ChangePublisher, HubChange},
    presentation::http::routes::create_router,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

fn app() -> Router {
    create_router(support::lazy_state(false), CorsLayer::new())
}
async fn body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 2_100_000).await.unwrap()).unwrap()
}

#[tokio::test]
async fn metadata_and_health_keep_separate_contracts() {
    let response = app()
        .oneshot(Request::get("/meta").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let meta = body(response).await;
    assert_eq!(meta["time_zone"], "Europe/Moscow");
    assert_eq!(meta["schedule_time_basis"], "hub_local");
    assert_eq!(meta["event_stream"]["replay"], false);
    let response = app()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body(response).await,
        json!({"status":"degraded","mqtt":"disconnected"})
    );
}

#[tokio::test]
async fn errors_include_stable_codes_without_leaking_internal_diagnostics() {
    for (uri, method, payload, status, code) in [
        (
            "/devices/plug/schedules",
            "POST",
            r#"{"command":"explode","run_at":"2026-09-05 10:00:00"}"#,
            400,
            "invalid_command",
        ),
        (
            "/devices/window/position",
            "POST",
            r#"{"position":101}"#,
            400,
            "invalid_position",
        ),
        (
            "/devices/window/recurring-commands",
            "POST",
            r#"{"command":"set_position","payload":{"position":-1},"local_time":"09:00"}"#,
            400,
            "invalid_position",
        ),
        (
            "/devices/plug/schedules",
            "POST",
            "{",
            400,
            "invalid_request",
        ),
        (
            "/devices/plug/schedules",
            "POST",
            "{}",
            422,
            "invalid_request",
        ),
        (
            "/devices/plug/events?limit=oops",
            "GET",
            "",
            400,
            "invalid_request",
        ),
        ("/schedules/nope", "DELETE", "", 400, "invalid_request"),
        ("/missing", "GET", "", 404, "not_found"),
        ("/meta", "POST", "{}", 405, "method_not_allowed"),
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .method(method)
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status, "{uri}");
        let data = body(response).await;
        assert_eq!(data["error"]["code"], code, "{uri}");
        assert!(data["error"]["message"].as_str().unwrap().len() > 5);
    }
    let state = support::lazy_state(true);
    let mut receiver = state.changes.subscribe();
    let response = create_router(state, CorsLayer::new())
        .oneshot(
            Request::post("/devices/plug/turn-on")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let data = body(response).await;
    assert_eq!(data["error"]["code"], "command_delivery_failed");
    assert!(!data.to_string().contains("private gateway diagnostic"));
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn body_and_media_type_rejections_are_json() {
    for (content_type, payload, status, code) in [
        (
            "text/plain",
            "{}".to_string(),
            415,
            "unsupported_media_type",
        ),
        (
            "application/json",
            " ".repeat(2_100_000),
            413,
            "payload_too_large",
        ),
    ] {
        let response = app()
            .oneshot(
                Request::post("/devices/plug/schedules")
                    .header("content-type", content_type)
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(body(response).await["error"]["code"], code);
    }
}

#[tokio::test]
async fn commands_from_http_and_other_clients_publish_the_same_notifications() {
    let state = support::lazy_state(false);
    let service = state.app_service.clone();
    let mut receiver = state.changes.subscribe();
    service.turn_on("plug").unwrap(); // A bot can invoke the application use case directly.
    let direct = serde_json::to_value(receiver.recv().await.unwrap()).unwrap();
    let response = create_router(state, CorsLayer::new())
        .oneshot(
            Request::post("/devices/plug/turn-on")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(
        to_bytes(response.into_body(), 1024)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        direct,
        serde_json::to_value(receiver.recv().await.unwrap()).unwrap()
    );
    assert_eq!(
        direct,
        json!({"kind":"command_accepted","device_id":"plug"})
    );
}

async fn frame(body: &mut Body) -> String {
    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}

#[tokio::test]
async fn sse_frames_support_resync_filtering_and_slow_clients() {
    let state = support::lazy_state(false);
    let publisher = state.changes.clone();
    let response = create_router(state, CorsLayer::new())
        .oneshot(
            Request::get("/events/stream?device_id=plug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut stream = response.into_body();
    let first = frame(&mut stream).await;
    assert!(first.contains("event: resync") && first.contains("connected"));
    publisher.publish(HubChange {
        kind: ChangeKind::DevicesChanged,
        device_id: Some("other".into()),
    });
    publisher.publish(HubChange {
        kind: ChangeKind::DevicesChanged,
        device_id: Some("plug".into()),
    });
    let change = frame(&mut stream).await;
    assert!(
        change.contains("event: change") && change.contains("plug") && !change.contains("other")
    );
    for _ in 0..300 {
        publisher.publish(HubChange {
            kind: ChangeKind::SchedulesChanged,
            device_id: None,
        });
    }
    let lag = frame(&mut stream).await;
    assert!(lag.contains("event: resync") && lag.contains("lagged"));
    assert!(frame(&mut stream).await.contains("schedules_changed"));
}
