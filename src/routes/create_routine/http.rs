//! `POST /routines` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{extract::State, http::StatusCode, Json};
use logic::{CreateRoutineRequest, RoutineResponse, RoutineStore};

/// `POST /routines` — create a new routine.
#[utoipa::path(post, path = "/routines",
    request_body = CreateRoutineRequest,
    responses((status = 201, body = RoutineResponse), (status = 400, description = "Invalid cron expression")))]
pub async fn create_routine(
    State(store): State<RoutineStore>,
    Json(body): Json<CreateRoutineRequest>,
) -> Result<(StatusCode, Json<RoutineResponse>), AppError> {
    // `svc_create` syncs the crontab, which shells out to `crontab`(1) (#360) — keep that
    // off the async worker thread.
    let resp = tokio::task::spawn_blocking(move || logic::build(&store, body))
        .await
        .map_err(|_| AppError::Internal)??;
    Ok((StatusCode::CREATED, Json(resp)))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
