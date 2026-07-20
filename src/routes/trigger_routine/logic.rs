//! Shared `trigger_routine` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{Routine, RoutineStore};

/// Manually trigger the routine with the given UUID, recording `last_manual_trigger_at` and
/// returning the updated record.
pub fn build(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    crate::routines::svc_trigger(store, id)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
