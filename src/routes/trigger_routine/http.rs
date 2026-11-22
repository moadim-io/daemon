//! `POST /routines/{id}/trigger` HTTP handler.

use super::logic;
use crate::error::{run_blocking, AppError};
use axum::{
    body::Bytes,
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
    request_body = Option<TriggerRoutineRequest>,
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn trigger_routine(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Routine>, AppError> {
    // `svc_trigger` shells out to `tmux`(1) (overlap guard, concurrency cap, session spawn) and
    // does blocking fs I/O — keep that off the async worker thread (#360), same as create/update/
    // delete.
    let override_system_power_saving = parse_options(&body)?.override_system_power_saving;
    let resp =
        run_blocking(move || logic::build(&store, &id, override_system_power_saving)).await?;
    Ok(Json(resp))
}

/// Decode the optional request body. An empty (or whitespace-only) body means default options,
/// whatever `Content-Type` the client sent: `Option<Json<_>>` only treats a *missing* content type
/// as "no body", so the CLI's `Content-Type: application/json` + empty body used to get a 400.
fn parse_options(body: &[u8]) -> Result<TriggerRoutineRequest, AppError> {
    if body.trim_ascii().is_empty() {
        return Ok(TriggerRoutineRequest::default());
    }
    serde_json::from_slice(body)
        .map_err(|err| AppError::BadRequest(format!("invalid trigger request body: {err}")))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
