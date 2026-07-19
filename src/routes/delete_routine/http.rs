//! `DELETE /routines/{id}` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{RoutineResponse, RoutineStore};

/// `DELETE /routines/{id}` — delete a routine by UUID.
#[utoipa::path(delete, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = RoutineResponse), (status = 404, description = "Not found")))]
pub async fn delete_routine(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<RoutineResponse>, AppError> {
    // `svc_delete` syncs the crontab (#360); keep it off the async executor thread.
    let resp = tokio::task::spawn_blocking(move || logic::build(&store, &id))
        .await
        .map_err(|_| AppError::Internal)??;
    Ok(Json(resp))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
