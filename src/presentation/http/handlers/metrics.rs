use axum::{
    body::Body,
    extract::State,
    http::{Request, header},
    middleware::Next,
    response::Response,
};

use crate::{observability::metrics::Metrics, presentation::http::state::AppState};

pub async fn metrics(
    State(state): State<AppState>,
) -> ([(header::HeaderName, &'static str); 1], String) {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        state.metrics.render_prometheus(),
    )
}

pub async fn track_http(
    State(metrics): State<Metrics>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let response = next.run(request).await;
    metrics.record_http_status(response.status());
    response
}
