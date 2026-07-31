//! REST endpoint for retrying OS crontab synchronization on demand.

use super::health::logic::{self, HealthResponse};
use super::http::AppState;
use crate::error::{run_blocking, AppError};
use axum::{extract::State, Json};

/// `POST /crontab-sync/retry` — retry writing the managed routines block to the OS crontab.
///
/// Returns the same health payload shape as `GET /health` after the retry. Crontab command
/// failures are reported inside `crontab_sync` rather than as HTTP 500 so the client can render
/// actionable recovery guidance and keep the daemon reachable.
#[utoipa::path(post, path = "/crontab-sync/retry",
    responses((status = 200, body = HealthResponse)))]
pub async fn retry_crontab_sync(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, AppError> {
    let store = state.routines.clone();
    let uptime_start = state.uptime_start;
    let response = run_blocking(move || {
        let _ = crate::sync::routines::sync_routines_to_crontab(&store);
        Ok(logic::build(uptime_start))
    })
    .await?;
    Ok(Json(response))
}

#[cfg(test)]
#[path = "retry_crontab_sync_tests.rs"]
mod tests;
