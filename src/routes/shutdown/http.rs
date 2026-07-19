//! `POST /shutdown` HTTP handler.

use super::logic;
use axum::{extract::State, Json};
use logic::ShutdownResponse;

/// `POST /shutdown` — ask the server to stop gracefully.
///
/// Used by the UI "STOP" button (and the `moadim stop` command) to kill a backgrounded server that
/// has no controlling terminal. The response is sent before the graceful shutdown completes.
#[utoipa::path(post, path = "/shutdown",
    responses((status = 200, body = ShutdownResponse)))]
pub async fn shutdown(
    State(state): State<crate::routes::http::AppState>,
) -> Json<ShutdownResponse> {
    log::info!("shutdown requested via API");
    Json(logic::build(&state.shutdown))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
