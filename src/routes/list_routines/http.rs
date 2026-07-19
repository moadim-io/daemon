//! `GET /routines` HTTP handler.

use super::logic;
use axum::{extract::Query, extract::State, Json};
use logic::{RoutineListQuery, RoutineResponse};

/// `GET /routines` — list routines, optionally filtered and sorted by repository.
#[utoipa::path(get, path = "/routines",
    params(RoutineListQuery),
    responses((status = 200, body = Vec<RoutineResponse>)))]
pub async fn list_routines(
    State(state): State<crate::routes::http::AppState>,
    Query(query): Query<RoutineListQuery>,
) -> Json<Vec<RoutineResponse>> {
    Json(logic::build(&state.routines, &state.routines_dir, &query))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
