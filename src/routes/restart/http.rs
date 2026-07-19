//! `POST /restart` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::Json;
use logic::RestartResponse;

/// `POST /restart` — stop this server and start a fresh instance.
///
/// The running server cannot rebind its own port, so it spawns a detached `moadim restart` helper
/// that stops it and starts a new process, mirroring the `moadim restart` CLI command and the
/// `restart` MCP tool. Responds with the helper's PID before the restart completes.
#[utoipa::path(post, path = "/restart",
    responses((status = 200, body = RestartResponse), (status = 500, description = "could not spawn the restart helper")))]
pub async fn restart() -> Result<Json<RestartResponse>, AppError> {
    Ok(Json(logic::build().map_err(|_| AppError::Internal)?))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
