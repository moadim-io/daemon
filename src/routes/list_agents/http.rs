//! `GET /agents` HTTP handler.

use super::logic;
use axum::Json;

/// `GET /agents` — list the agent registry keys a routine may target.
#[utoipa::path(get, path = "/agents",
    responses((status = 200, body = Vec<String>, description = "Available agent names")))]
pub async fn list_agents() -> Json<Vec<String>> {
    Json(logic::build())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
