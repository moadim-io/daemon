//! Axum HTTP handlers for the `/routines` resource.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::error::AppError;

use super::ical::{svc_ical, svc_ical_routine};
use super::model::{
    CleanupResponse, CreateRoutineRequest, IcalFeedQuery, Routine, RoutineListQuery,
    RoutineResponse, RoutineStore, UpdateRoutineRequest,
};
use super::service::{
    svc_cleanup, svc_create, svc_delete, svc_get, svc_list, svc_logs, svc_trigger, svc_update,
};

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

/// `PUT /routines/{id}` — fully replace a routine (behaves identically to PATCH).
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

/// `GET /routines.ics` — iCalendar feed of upcoming routine fire times.
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
