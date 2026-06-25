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
fn display_conflict() {
    assert_eq!(
        AppError::Conflict("duplicate".into()).to_string(),
        "conflict: duplicate"
    );
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

#[test]
fn into_response_conflict_is_409() {
    assert_eq!(
        AppError::Conflict("x".into()).into_response().status(),
        StatusCode::CONFLICT
    );
}

#[test]
fn display_locked() {
    assert_eq!(
        AppError::Locked("routines are globally locked".into()).to_string(),
        "locked: routines are globally locked"
    );
}

#[test]
fn into_response_locked_is_423() {
    assert_eq!(
        AppError::Locked("x".into()).into_response().status(),
        StatusCode::LOCKED
    );
}

#[tokio::test]
async fn into_response_body_carries_locked_message() {
    assert_eq!(
        response_error_field(AppError::Locked("paused".into())).await,
        "locked: paused"
    );
}

/// Decode an [`AppError`] response body into its `{"error": ...}` JSON.
async fn response_error_field(err: AppError) -> String {
    let body = err.into_response().into_body();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["error"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn into_response_body_carries_bad_request_message() {
    assert_eq!(
        response_error_field(AppError::BadRequest("oops".into())).await,
        "bad request: oops"
    );
}

#[tokio::test]
async fn into_response_body_carries_conflict_message() {
    assert_eq!(
        response_error_field(AppError::Conflict("duplicate".into())).await,
        "conflict: duplicate"
    );
}

#[tokio::test]
async fn into_response_body_carries_not_found_message() {
    assert_eq!(response_error_field(AppError::NotFound).await, "not found");
}

#[tokio::test]
async fn into_response_body_carries_internal_message() {
    assert_eq!(
        response_error_field(AppError::Internal).await,
        "internal server error"
    );
}
