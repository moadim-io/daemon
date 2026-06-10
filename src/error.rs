use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Internal,
    BadRequest(String),
    NotFound,
    Conflict(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Internal => write!(f, "internal server error"),
            AppError::BadRequest(msg) => write!(f, "bad request: {}", msg),
            AppError::NotFound => write!(f, "not found"),
            AppError::Conflict(msg) => write!(f, "conflict: {}", msg),
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
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
