//! `GET /routines/{id}` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Path, State},
    Json,
};
use logic::RoutineResponse;

/// `GET /routines/{id}` — retrieve a single routine by UUID.
#[utoipa::path(get, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = RoutineResponse), (status = 404, description = "Not found")))]
pub async fn get_routine(
    State(state): State<crate::routes::http::AppState>,
    Path(id): Path<String>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(logic::build(
        &state.routines,
        &state.routines_dir,
        &id,
    )?))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
