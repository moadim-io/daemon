//! `PATCH /routines/{id}` and `PUT /routines/{id}` HTTP handlers.

use super::logic;
use crate::error::{run_blocking, AppError};
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{RoutineResponse, RoutineStore, UpdateRoutineRequest};

/// `PATCH /routines/{id}` — partially update a routine.
#[utoipa::path(patch, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = UpdateRoutineRequest,
    responses((status = 200, body = RoutineResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update_routine(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRoutineRequest>,
) -> Result<Json<RoutineResponse>, AppError> {
    // `svc_update` syncs the crontab, which shells out to `crontab`(1) (#360) — keep that off the
    // async worker thread.
    let resp = run_blocking(move || logic::build(&store, &id, body)).await?;
    Ok(Json(resp))
}

/// `PUT /routines/{id}` — alias for `PATCH`: a partial-merge update, not a full replace.
///
/// Fields omitted from the body are retained from the existing record, exactly as with `PATCH`.
/// A client expecting RFC 7231 full-resource-replacement semantics (omitted fields reset to
/// default) should not rely on this route for that; use `PATCH` and set every field explicitly.
#[utoipa::path(put, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = UpdateRoutineRequest,
    responses((status = 200, body = RoutineResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn replace(
    state: State<RoutineStore>,
    path: Path<String>,
    body: Json<UpdateRoutineRequest>,
) -> Result<Json<RoutineResponse>, AppError> {
    update_routine(state, path, body).await
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
