//! Axum HTTP handlers for the `/routines` resource.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::global_lock::{LockScope, LockStatus};

use super::flags::Flag;
use super::ical::{svc_ical, svc_ical_routine};
use super::model::{
    CleanupResponse, CreateRoutineRequest, FleetRunSummary, IcalFeedQuery, Routine,
    RoutineListQuery, RoutineResponse, RoutineStore, RunSummary, UpdateRoutineRequest,
};
use super::service::{
    svc_cleanup, svc_create, svc_create_flag, svc_delete, svc_get, svc_list, svc_list_all_runs,
    svc_list_flags, svc_list_runs, svc_logs, svc_resolve_flag, svc_run_log, svc_trigger,
    svc_trigger_scheduled, svc_update,
};

/// Request body for `POST /routines/{id}/flags`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateFlagRequest {
    /// Free-text flag category. Common examples: `"bug"`, `"gap"`, `"edge_case"`, `"question"`,
    /// `"blocker"` — any string is accepted.
    #[serde(rename = "type")]
    pub flag_type: String,
    /// Free-text description of what's unclear.
    pub description: String,
    /// `"general"` (committed, shared via git) or `"local"` (gitignored, machine-local).
    pub scope: String,
}

/// Request body for `POST /routines/lock`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LockRequest {
    /// Which sentinel to create: `"shared"` (committed `.lock`) or `"local"` (gitignored `.local.lock`).
    pub scope: String,
}

/// Query parameters for `DELETE /routines/lock`.
#[derive(Deserialize, utoipa::IntoParams)]
pub struct UnlockQuery {
    /// Which sentinel(s) to remove: `"shared"`, `"local"`, or `"all"`.
    pub scope: String,
}

/// `GET /routines/lock` — return the current global lock status.
#[utoipa::path(get, path = "/routines/lock",
    responses((status = 200, body = LockStatus)))]
pub async fn get_lock_status() -> Json<LockStatus> {
    Json(crate::global_lock::lock_status())
}

/// `POST /routines/lock` — create a lock sentinel, halting all routine scheduling and triggers.
#[utoipa::path(post, path = "/routines/lock",
    request_body = LockRequest,
    responses((status = 200, body = LockStatus), (status = 400, description = "Unknown scope"), (status = 500, description = "IO error")))]
pub async fn lock(
    State(store): State<RoutineStore>,
    Json(body): Json<LockRequest>,
) -> Result<Json<LockStatus>, AppError> {
    let scope = parse_lock_scope(&body.scope)?;
    crate::global_lock::set_lock(scope, true).map_err(|_| AppError::Internal)?;
    if let Err(sync_err) = crate::sync::routines::sync_routines_to_crontab(&store) {
        log::warn!("crontab sync after HTTP lock failed: {sync_err}");
    }
    Ok(Json(crate::global_lock::lock_status()))
}

/// `DELETE /routines/lock` — remove lock sentinel(s), restoring routine scheduling.
#[utoipa::path(delete, path = "/routines/lock",
    params(UnlockQuery),
    responses((status = 200, body = LockStatus), (status = 400, description = "Unknown scope"), (status = 500, description = "IO error")))]
pub async fn unlock(
    State(store): State<RoutineStore>,
    Query(query): Query<UnlockQuery>,
) -> Result<Json<LockStatus>, AppError> {
    let scopes: Vec<LockScope> = if query.scope == "all" {
        vec![LockScope::Shared, LockScope::Local]
    } else {
        vec![parse_lock_scope(&query.scope)?]
    };
    for scope in scopes {
        crate::global_lock::set_lock(scope, false).map_err(|_| AppError::Internal)?;
    }
    if let Err(sync_err) = crate::sync::routines::sync_routines_to_crontab(&store) {
        log::warn!("crontab sync after HTTP unlock failed: {sync_err}");
    }
    Ok(Json(crate::global_lock::lock_status()))
}

/// Parse a `scope` string into a [`LockScope`], returning `400 BadRequest` on unknown values.
fn parse_lock_scope(scope: &str) -> Result<LockScope, AppError> {
    match scope {
        "shared" => Ok(LockScope::Shared),
        "local" => Ok(LockScope::Local),
        other => Err(AppError::BadRequest(format!(
            "unknown scope {other:?}; use \"shared\" or \"local\""
        ))),
    }
}

/// `POST /routines` — create a new routine.
#[utoipa::path(post, path = "/routines",
    request_body = CreateRoutineRequest,
    responses((status = 201, body = RoutineResponse), (status = 400, description = "Invalid cron expression")))]
pub async fn create(
    State(store): State<RoutineStore>,
    Json(body): Json<CreateRoutineRequest>,
) -> Result<(StatusCode, Json<RoutineResponse>), AppError> {
    Ok((StatusCode::CREATED, Json(svc_create(&store, body)?)))
}

/// `GET /routines` — list routines, optionally filtered and sorted by repository.
#[utoipa::path(get, path = "/routines",
    params(RoutineListQuery),
    responses((status = 200, body = Vec<RoutineResponse>)))]
pub async fn list(
    State(store): State<RoutineStore>,
    Query(query): Query<RoutineListQuery>,
) -> Json<Vec<RoutineResponse>> {
    Json(svc_list(&store, &query))
}

/// `GET /agents` — list the agent registry keys a routine may target.
#[utoipa::path(get, path = "/agents",
    responses((status = 200, body = Vec<String>, description = "Available agent names")))]
pub async fn list_agents() -> Json<Vec<String>> {
    Json(super::available_agents())
}

/// `GET /routines/{id}` — retrieve a single routine by UUID.
#[utoipa::path(get, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = RoutineResponse), (status = 404, description = "Not found")))]
pub async fn get(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(svc_get(&store, &id)?))
}

/// `PATCH /routines/{id}` — partially update a routine.
#[utoipa::path(patch, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = UpdateRoutineRequest,
    responses((status = 200, body = RoutineResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRoutineRequest>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(svc_update(&store, &id, body)?))
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
    update(state, path, body).await
}

/// `DELETE /routines/{id}` — delete a routine by UUID.
#[utoipa::path(delete, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = RoutineResponse), (status = 404, description = "Not found")))]
pub async fn delete(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(svc_delete(&store, &id)?))
}

/// `POST /routines/{id}/trigger` — manually run a routine outside its schedule.
#[utoipa::path(post, path = "/routines/{id}/trigger",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn trigger(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Routine>, AppError> {
    Ok(Json(svc_trigger(&store, &id)?))
}

/// `POST /routines/{id}/scheduled-trigger` — run a routine on its schedule.
///
/// The daemon-side endpoint the generated crontab line invokes (`moadim schedule trigger <id>`).
/// Unlike [`trigger`] it does not record a manual trigger; the spawned command records the scheduled
/// timestamp itself. See [`svc_trigger_scheduled`].
#[utoipa::path(post, path = "/routines/{id}/scheduled-trigger",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn scheduled_trigger(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Routine>, AppError> {
    Ok(Json(svc_trigger_scheduled(&store, &id)?))
}

/// `GET /routines.ics` — iCalendar feed of every enabled routine's upcoming fire times.
///
/// Returns a `text/calendar` body suitable for subscribing to in an external calendar
/// (Google Calendar, Apple Calendar, …) so upcoming runs show up alongside other events.
/// With `?routine=<id>` the feed is scoped to a single routine (named after it); without
/// it every enabled routine is rendered (issue #263).
#[utoipa::path(get, path = "/routines.ics",
    params(IcalFeedQuery),
    responses((status = 200, description = "iCalendar (text/calendar) feed of upcoming routine fire times")))]
pub async fn ical_feed(
    State(store): State<RoutineStore>,
    Query(query): Query<IcalFeedQuery>,
) -> impl IntoResponse {
    let body = match query.routine.as_deref() {
        Some(id) => svc_ical_routine(&store, id),
        None => svc_ical(&store),
    };
    (
        [(header::CONTENT_TYPE, "text/calendar; charset=utf-8")],
        body,
    )
}

/// `POST /routines/cleanup` — reap finished, expired run workbenches on demand.
#[utoipa::path(post, path = "/routines/cleanup",
    responses((status = 200, body = CleanupResponse, description = "Number of workbenches removed")))]
pub async fn cleanup(State(store): State<RoutineStore>) -> Json<CleanupResponse> {
    Json(svc_cleanup(&store))
}

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
    let flag = svc_create_flag(&store, &id, &body.flag_type, &body.description, &body.scope)?;
    Ok((StatusCode::CREATED, Json(flag)))
}

/// `GET /routines/{id}/flags` — list open flags raised against a routine.
#[utoipa::path(get, path = "/routines/{id}/flags",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Vec<Flag>), (status = 404, description = "Not found")))]
pub async fn list_flags(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Flag>>, AppError> {
    Ok(Json(svc_list_flags(&store, &id)?))
}

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
    svc_resolve_flag(&store, &id, &filename)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /routines/{id}/logs` — return the newest workbench `agent.log` as plain text.
#[utoipa::path(get, path = "/routines/{id}/logs",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, description = "Log file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_logs(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<String, AppError> {
    svc_logs(&store, &id)
}

/// `GET /routines/{id}/runs` — list every run workbench for the routine, newest first.
#[utoipa::path(get, path = "/routines/{id}/runs",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = [RunSummary]), (status = 404, description = "Not found")))]
pub async fn get_runs(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Vec<RunSummary>>, AppError> {
    svc_list_runs(&store, &id).map(Json)
}

/// Query parameters for `GET /routines/runs`.
#[derive(Deserialize, utoipa::IntoParams)]
pub struct FleetRunsQuery {
    /// Cap on the number of runs returned (default: `DEFAULT_FLEET_RUNS_LIMIT`).
    pub limit: Option<usize>,
}

/// `GET /routines/runs` — the most recent runs across every routine, newest first. Backs the
/// overview "recent runs" panel with a single workbench scan instead of one request per routine.
#[utoipa::path(get, path = "/routines/runs",
    params(FleetRunsQuery),
    responses((status = 200, body = [FleetRunSummary])))]
pub async fn get_all_runs(
    State(store): State<RoutineStore>,
    Query(query): Query<FleetRunsQuery>,
) -> Json<Vec<FleetRunSummary>> {
    Json(svc_list_all_runs(&store, query.limit))
}

/// `GET /routines/{id}/runs/{workbench}/log` — return one specific run's `agent.log` as plain text.
#[utoipa::path(get, path = "/routines/{id}/runs/{workbench}/log",
    params(
        ("id" = String, Path, description = "Routine UUID"),
        ("workbench" = String, Path, description = "Workbench directory name (`{slug}-{unix_secs}`), from `GET /routines/{id}/runs`"),
    ),
    responses((status = 200, description = "Log file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_run_log(
    State(store): State<RoutineStore>,
    Path((id, workbench)): Path<(String, String)>,
) -> Result<String, AppError> {
    svc_run_log(&store, &id, &workbench)
}

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod handlers_tests;
