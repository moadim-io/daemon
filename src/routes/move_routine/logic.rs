//! Shared `move_routine` logic for HTTP and MCP.

use crate::error::AppError;
pub(crate) use crate::routines::{MoveRoutineRequest, RoutineResponse, RoutineStore};

/// Move a routine's filesystem directory explicitly.
pub fn build(
    store: &RoutineStore,
    id: &str,
    req: MoveRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    crate::routines::svc_move(store, id, req)
}
