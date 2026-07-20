//! `POST /routines/lock` HTTP handler.

use super::logic;
use crate::error::{run_blocking, AppError};
use axum::{extract::State, Json};
use logic::{LockRequest, LockStatus, RoutineStore};

/// `POST /routines/lock` — create a lock sentinel, halting all routine scheduling and triggers.
#[utoipa::path(post, path = "/routines/lock",
    request_body = LockRequest,
    responses((status = 200, body = LockStatus), (status = 400, description = "Unknown scope"), (status = 500, description = "IO error")))]
pub async fn lock_routines(
    State(store): State<RoutineStore>,
    Json(body): Json<LockRequest>,
) -> Result<Json<LockStatus>, AppError> {
    // Crontab sync shells out to `crontab`(1); run it on the blocking pool so a slow or
    // hung invocation can't pin a Tokio worker thread (#360).
    let resp = run_blocking(move || logic::build(&store, &body.scope)).await?;
    Ok(Json(resp))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
