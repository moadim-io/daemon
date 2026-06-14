#![allow(clippy::missing_docs_in_private_items)]

use axum::http::StatusCode;
use axum::response::IntoResponse;

use super::*;

#[test]
fn display_internal() {
    assert_eq!(AppError::Internal.to_string(), "internal server error");
}

#[test]
fn display_bad_request() {
    assert_eq!(
        AppError::BadRequest("oops".into()).to_string(),
        "bad request: oops"
    );
}

#[test]
fn display_not_found() {
    assert_eq!(AppError::NotFound.to_string(), "not found");
}

#[test]
fn into_response_internal_is_500() {
    assert_eq!(
        AppError::Internal.into_response().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn into_response_bad_request_is_400() {
    assert_eq!(
        AppError::BadRequest("x".into()).into_response().status(),
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn into_response_not_found_is_404() {
    assert_eq!(
        AppError::NotFound.into_response().status(),
        StatusCode::NOT_FOUND
    );
}
