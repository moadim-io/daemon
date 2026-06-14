//! Axum HTTP handlers for the `/routines` resource.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;

use super::model::{
    CreateRoutineRequest, Routine, RoutineResponse, RoutineStore, UpdateRoutineRequest,
};
use super::service::{svc_create, svc_delete, svc_get, svc_list, svc_logs, svc_trigger, svc_update};

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

/// `GET /routines` — list all routines sorted by creation time.
#[utoipa::path(get, path = "/routines",
    responses((status = 200, body = Vec<RoutineResponse>)))]
pub async fn list(State(store): State<RoutineStore>) -> Json<Vec<RoutineResponse>> {
    Json(svc_list(&store))
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
