//! `DELETE /routines/{id}/flags/{filename}` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use logic::RoutineStore;

/// `DELETE /routines/{id}/flags/{filename}` — resolve (delete) a flag.
#[utoipa::path(delete, path = "/routines/{id}/flags/{filename}",
    params(
        ("id" = String, Path, description = "Routine UUID"),
        ("filename" = String, Path, description = "Flag filename, as returned by create/list"),
    ),
    responses((status = 204, description = "Resolved"), (status = 404, description = "Not found")))]
pub async fn resolve_flag(
    State(store): State<RoutineStore>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    logic::build(&store, &id, &filename)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
