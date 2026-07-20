//! `POST /routines/{id}/trigger` HTTP handler.

use super::logic;
use crate::error::{run_blocking, AppError};
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{Routine, RoutineStore};

/// `POST /routines/{id}/trigger` — manually trigger a routine outside its schedule.
///
/// Refuses (423, distinct message) when the routine is disabled or in power-saving mode. See
/// [`crate::routines::svc_trigger`].
#[utoipa::path(post, path = "/routines/{id}/trigger",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn trigger_routine(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Routine>, AppError> {
    // `svc_trigger` shells out to `tmux`(1) (overlap guard, concurrency cap, session spawn) and
    // does blocking fs I/O — keep that off the async worker thread (#360), same as create/update/
    // delete.
    let resp = run_blocking(move || logic::build(&store, &id)).await?;
    Ok(Json(resp))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
