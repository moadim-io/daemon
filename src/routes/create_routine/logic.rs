//! Shared `create_routine` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{CreateRoutineRequest, RoutineResponse, RoutineStore};

/// Validate and persist a new routine, returning the created record.
pub fn build(store: &RoutineStore, req: CreateRoutineRequest) -> Result<RoutineResponse, AppError> {
    crate::routines::svc_create(store, req)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
