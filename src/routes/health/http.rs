//! `GET /health` HTTP handler.

use super::logic;
use axum::{extract::State, Json};
use logic::HealthResponse;

/// `GET /health` — health check with uptime.
#[utoipa::path(get, path = "/health",
    responses((status = 200, body = HealthResponse)))]
pub async fn health(State(state): State<crate::routes::http::AppState>) -> Json<HealthResponse> {
    Json(logic::build(state.uptime_start))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
