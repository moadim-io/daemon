//! Axum HTTP handlers for cron job CRUD and trigger endpoints.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::cron_jobs::{
    svc_create, svc_delete, svc_get, svc_list, svc_trigger, svc_update, AppState, CronJob,
    CronJobResponse, CronStore, CreateRequest, UpdateRequest,
};
use crate::error::AppError;

/// `POST /cron-jobs` — create a new cron job.
#[utoipa::path(post, path = "/cron-jobs",
    request_body = CreateRequest,
    responses((status = 201, body = CronJobResponse), (status = 400, description = "Invalid cron expression")))]
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CronJobResponse>), AppError> {
    Ok((StatusCode::CREATED, Json(svc_create(&state.store, &state.handlers, body)?)))
}

/// `GET /cron-jobs` — list all cron jobs sorted by creation time.
#[utoipa::path(get, path = "/cron-jobs",
    responses((status = 200, body = Vec<CronJobResponse>)))]
pub async fn list(State(state): State<AppState>) -> Json<Vec<CronJobResponse>> {
    Json(svc_list(&state.store, &state.handlers))
}

/// `GET /cron-jobs/{id}` — retrieve a single cron job by UUID.
#[utoipa::path(get, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJobResponse), (status = 404, description = "Not found")))]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CronJobResponse>, AppError> {
    Ok(Json(svc_get(&state.store, &state.handlers, &id)?))
}

/// `PATCH /cron-jobs/{id}` — partially update a cron job.
#[utoipa::path(patch, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    request_body = UpdateRequest,
    responses((status = 200, body = CronJobResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<CronJobResponse>, AppError> {
    Ok(Json(svc_update(&state.store, &state.handlers, &id, body)?))
}

/// `PUT /cron-jobs/{id}` — replace a cron job's fields (full update, equivalent to PATCH).
#[utoipa::path(put, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    request_body = UpdateRequest,
    responses(
        (status = 200, body = CronJobResponse),
        (status = 400, description = "Invalid"),
        (status = 404, description = "Not found")
    ))]
pub async fn replace(
    state: State<AppState>,
    path: Path<String>,
    body: Json<UpdateRequest>,
) -> Result<Json<CronJobResponse>, AppError> {
    update(state, path, body).await
}

/// `DELETE /cron-jobs/{id}` — delete a cron job by UUID.
#[utoipa::path(delete, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJobResponse), (status = 404, description = "Not found")))]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CronJobResponse>, AppError> {
    Ok(Json(svc_delete(&state.store, &state.handlers, &id)?))
}

/// `POST /cron-jobs/{id}/trigger` — manually trigger a cron job outside its schedule.
#[utoipa::path(post, path = "/cron-jobs/{id}/trigger",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJob), (status = 404, description = "Not found")))]
pub async fn trigger(
    State(store): State<CronStore>,
    Path(id): Path<String>,
) -> Result<Json<CronJob>, AppError> {
    Ok(Json(svc_trigger(&store, &id)?))
}
