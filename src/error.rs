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
    /// 423 Locked — a global lock sentinel is preventing the operation.
    Locked(String),
    /// 403 Forbidden — a request failed the `Host`/`Origin` allowlist check (issue #266).
    Forbidden(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal => write!(f, "internal server error"),
            Self::BadRequest(msg) => write!(f, "bad request: {msg}"),
            Self::NotFound => write!(f, "not found"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
            Self::Locked(msg) => write!(f, "locked: {msg}"),
            Self::Forbidden(msg) => write!(f, "forbidden: {msg}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Locked(_) => StatusCode::LOCKED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
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
