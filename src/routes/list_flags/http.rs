//! `GET /routines/{id}/flags` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{Flag, RoutineStore};

/// `GET /routines/{id}/flags` — list open flags raised against a routine.
#[utoipa::path(get, path = "/routines/{id}/flags",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Vec<Flag>), (status = 404, description = "Not found")))]
pub async fn list_flags(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Flag>>, AppError> {
    logic::build(&store, &id).map(Json)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
