//! `DELETE /routines/lock` HTTP handler.

use super::logic;
use crate::error::AppError;
use axum::{
    extract::{Query, State},
    Json,
};
use logic::{LockStatus, RoutineStore, UnlockQuery};

/// `DELETE /routines/lock` — remove lock sentinel(s), restoring routine scheduling.
#[utoipa::path(delete, path = "/routines/lock",
    params(UnlockQuery),
    responses((status = 200, body = LockStatus), (status = 400, description = "Unknown scope"), (status = 500, description = "IO error")))]
pub async fn unlock_routines(
    State(store): State<RoutineStore>,
    Query(query): Query<UnlockQuery>,
) -> Result<Json<LockStatus>, AppError> {
    // See `crate::routes::lock_routines::lock_routines`: crontab sync must not run inline on the
    // async worker thread (#360).
    let resp = tokio::task::spawn_blocking(move || logic::build(&store, &query.scope))
        .await
        .map_err(|_| AppError::Internal)??;
    Ok(Json(resp))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
