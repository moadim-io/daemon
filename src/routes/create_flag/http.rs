//! `POST /routines/{id}/flags` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use logic::{CreateFlagRequest, Flag, RoutineStore};

/// `POST /routines/{id}/flags` — raise a new flag against a routine.
#[utoipa::path(post, path = "/routines/{id}/flags",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = CreateFlagRequest,
    responses((status = 201, body = Flag), (status = 400, description = "Invalid type/description/scope"), (status = 404, description = "Not found")))]
pub async fn create_flag(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    Json(body): Json<CreateFlagRequest>,
) -> Result<(StatusCode, Json<Flag>), AppError> {
    let flag = logic::build(&store, &id, &body.flag_type, &body.description, &body.scope)?;
    Ok((StatusCode::CREATED, Json(flag)))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
