use actix_web::http::StatusCode;
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

impl actix_web::error::ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
        }
    }

    fn error_response(&self) -> actix_web::HttpResponse {
        let body = serde_json::json!({ "error": self.to_string() });
        actix_web::HttpResponse::build(self.status_code()).json(body)
    }
}

pub type AppResult<T> = Result<T, AppError>;
