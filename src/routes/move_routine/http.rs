//! `POST /routines/{id}/move` HTTP handler.

use super::logic;
use crate::error::{run_blocking, AppError};
use axum::{
    extract::{Path, State},
    Json,
};
use logic::{MoveRoutineRequest, RoutineResponse, RoutineStore};

/// `POST /routines/{id}/move` — explicitly move a routine directory under `routines/`.
#[utoipa::path(post, path = "/routines/{id}/move",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = MoveRoutineRequest,
    responses((status = 200, body = RoutineResponse), (status = 400, description = "Invalid path"), (status = 404, description = "Not found"), (status = 409, description = "Target exists")))]
pub async fn move_routine(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    Json(body): Json<MoveRoutineRequest>,
) -> Result<Json<RoutineResponse>, AppError> {
    let resp = run_blocking(move || logic::build(&store, &id, body)).await?;
    Ok(Json(resp))
}
