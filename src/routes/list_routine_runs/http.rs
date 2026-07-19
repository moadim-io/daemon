//! `GET /routines/{id}/runs` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{RoutineStore, RunSummary};

/// `GET /routines/{id}/runs` — list every run workbench for the routine, newest first.
#[utoipa::path(get, path = "/routines/{id}/runs",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = [RunSummary]), (status = 404, description = "Not found")))]
pub async fn list_routine_runs(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Vec<RunSummary>>, AppError> {
    logic::build(&store, &id).map(Json)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
