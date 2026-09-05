use axum::{
    Json,
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorDetails,
}

#[derive(Serialize)]
struct ErrorDetails {
    code: &'static str,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'static str>,
}

pub struct ApiError {
    status: StatusCode,
    details: ErrorDetails,
}

impl ApiError {
    pub fn invalid(code: &'static str, message: &'static str, field: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            details: ErrorDetails {
                code,
                message,
                field: Some(field),
            },
        }
    }

    pub fn from_status(status: StatusCode) -> Self {
        let (code, message) = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                ("invalid_request", "The request is invalid.")
            }
            StatusCode::NOT_FOUND => ("not_found", "The requested resource was not found."),
            StatusCode::METHOD_NOT_ALLOWED => {
                ("method_not_allowed", "This HTTP method is not supported.")
            }
            StatusCode::PAYLOAD_TOO_LARGE => {
                ("payload_too_large", "The request body is too large.")
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => (
                "unsupported_media_type",
                "Use application/json for the request body.",
            ),
            StatusCode::BAD_GATEWAY => (
                "command_delivery_failed",
                "The command could not be sent to the device gateway.",
            ),
            StatusCode::SERVICE_UNAVAILABLE => (
                "service_unavailable",
                "The service is temporarily unavailable.",
            ),
            _ => ("internal_error", "The request could not be completed."),
        };
        Self {
            status,
            details: ErrorDetails {
                code,
                message,
                field: None,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: self.details,
            }),
        )
            .into_response()
    }
}

/// Normalize handler and Axum extractor errors, keeping status codes and headers.
/// Health/metrics have independent operational contracts and remain unchanged.
pub async fn normalize_errors(request: Request, next: Next) -> Response {
    let operational = matches!(request.uri().path(), "/health" | "/metrics");
    let response = next.run(request).await;
    if operational
        || !response.status().is_client_error() && !response.status().is_server_error()
        || response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v == "application/json")
    {
        return response;
    }
    let error = ApiError::from_status(response.status()).into_response();
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, error.into_body())
}
