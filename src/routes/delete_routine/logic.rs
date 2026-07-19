//! Shared `delete_routine` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{RoutineResponse, RoutineStore};

/// Delete the routine with the given UUID, returning the deleted record.
pub fn build(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    crate::routines::svc_delete(store, id)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
