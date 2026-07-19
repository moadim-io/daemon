//! Shared `get_routine` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{RoutineResponse, RoutineStore};

/// Look up a single routine by UUID, reloading the store from disk first.
pub fn build(
    routines: &RoutineStore,
    routines_dir: &std::path::Path,
    id: &str,
) -> Result<RoutineResponse, AppError> {
    crate::routines::svc_get(routines, routines_dir, id)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
