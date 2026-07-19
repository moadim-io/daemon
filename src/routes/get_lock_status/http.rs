//! `GET /routines/lock` HTTP handler.

use super::logic;
use axum::Json;
use logic::LockStatus;

/// `GET /routines/lock` — return the current global lock status.
#[utoipa::path(get, path = "/routines/lock",
    responses((status = 200, body = LockStatus)))]
pub async fn get_lock_status() -> Json<LockStatus> {
    Json(logic::build())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
