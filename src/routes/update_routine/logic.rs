//! Shared `update_routine` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{RoutineResponse, RoutineStore, UpdateRoutineRequest};

/// Apply `req` to the routine identified by `id`, returning the updated record.
pub fn build(
    store: &RoutineStore,
    id: &str,
    req: UpdateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    crate::routines::svc_update(store, id, req)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
