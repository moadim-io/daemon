//! Application error type mapped to HTTP status codes.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::fmt;

/// Application-level error that converts to an HTTP response.
#[derive(Debug)]
pub enum AppError {
    /// 500 Internal Server Error.
    Internal,
    /// 400 Bad Request with a human-readable description.
    BadRequest(String),
    /// 404 Not Found.
    NotFound,
    /// 409 Conflict with a human-readable description.
    Conflict(String),
    /// 409 Conflict: the routine is user-disabled, so a manual trigger will not launch it (#95).
    /// Distinct from [`AppError::RoutinePowerSaving`] so callers can tell the deliberate user-off
    /// state from the transient system throttle.
    RoutineDisabled,
    /// 503 Service Unavailable: the routine is transiently paused by the system's power-saving
    /// throttle and will resume on its own (#95). Distinct from [`AppError::RoutineDisabled`].
    RoutinePowerSaving,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Internal => write!(f, "internal server error"),
            AppError::BadRequest(msg) => write!(f, "bad request: {msg}"),
            AppError::NotFound => write!(f, "not found"),
            AppError::Conflict(msg) => write!(f, "conflict: {msg}"),
            AppError::RoutineDisabled => {
                write!(f, "routine is disabled; enable it before triggering")
            }
            AppError::RoutinePowerSaving => write!(
                f,
                "routine is paused by power saving; it will resume automatically"
            ),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::RoutineDisabled => StatusCode::CONFLICT,
            AppError::RoutinePowerSaving => StatusCode::SERVICE_UNAVAILABLE,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
