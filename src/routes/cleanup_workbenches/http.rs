//! `POST /routines/cleanup` HTTP handler.

use super::logic;
use crate::routines::RoutineStore;
use axum::{extract::State, Json};
use logic::CleanupResponse;

/// `POST /routines/cleanup` — reap finished, expired run workbenches on demand.
#[utoipa::path(post, path = "/routines/cleanup",
    responses((status = 200, body = CleanupResponse, description = "Workbenches removed and bytes freed")))]
pub async fn cleanup_workbenches(State(store): State<RoutineStore>) -> Json<CleanupResponse> {
    // `logic::build` does blocking fs scans and shells out to `tmux`(1) to kill hung sessions
    // (#360) — the background hourly sweep (`http_listener::cleanup_task`) already runs this on
    // `spawn_blocking`; this on-demand endpoint should not run it inline on the worker thread
    // either.
    Json(
        tokio::task::spawn_blocking(move || logic::build(&store))
            .await
            .unwrap_or(CleanupResponse {
                removed: 0,
                freed_bytes: 0,
            }),
    )
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
