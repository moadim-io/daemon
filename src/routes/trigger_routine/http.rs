//! `POST /routines/{id}/trigger` HTTP handler.

use super::logic;
use crate::error::{run_blocking, AppError};
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{Routine, RoutineStore, TriggerRoutineRequest};

/// `POST /routines/{id}/trigger` — manually trigger a routine outside its schedule.
///
/// Refuses (423, distinct message) when the routine is disabled or in power-saving mode. A
/// confirmed UI request may bypass host-level power saving for that one manual run. See
/// [`crate::routines::svc_trigger`].
#[utoipa::path(post, path = "/routines/{id}/trigger",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = TriggerRoutineRequest,
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn trigger_routine(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    body: Option<Json<TriggerRoutineRequest>>,
) -> Result<Json<Routine>, AppError> {
    // `svc_trigger` shells out to `tmux`(1) (overlap guard, concurrency cap, session spawn) and
    // does blocking fs I/O — keep that off the async worker thread (#360), same as create/update/
    // delete.
    let override_system_power_saving =
        body.is_some_and(|Json(body)| body.override_system_power_saving);
    let resp =
        run_blocking(move || logic::build(&store, &id, override_system_power_saving)).await?;
    Ok(Json(resp))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
