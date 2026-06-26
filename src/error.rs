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
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Internal => write!(f, "internal server error"),
            AppError::BadRequest(msg) => write!(f, "bad request: {msg}"),
            AppError::NotFound => write!(f, "not found"),
            AppError::Conflict(msg) => write!(f, "conflict: {msg}"),
            AppError::Locked(msg) => write!(f, "locked: {msg}"),
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
            AppError::Locked(_) => StatusCode::LOCKED,
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
