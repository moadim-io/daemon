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
fn display_routine_disabled() {
    assert_eq!(
        AppError::RoutineDisabled.to_string(),
        "routine is disabled; enable it before triggering"
    );
}

#[test]
fn display_routine_power_saving() {
    assert_eq!(
        AppError::RoutinePowerSaving.to_string(),
        "routine is paused by power saving; it will resume automatically"
    );
}

#[test]
fn into_response_routine_disabled_is_409() {
    assert_eq!(
        AppError::RoutineDisabled.into_response().status(),
        StatusCode::CONFLICT
    );
}

#[test]
fn into_response_routine_power_saving_is_503() {
    assert_eq!(
        AppError::RoutinePowerSaving.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
