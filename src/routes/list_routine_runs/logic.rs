//! Shared `list_routine_runs` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{RoutineStore, RunSummary};

/// List every run for routine `id`, newest first (live workbenches plus durable history).
pub fn build(routines: &RoutineStore, id: &str) -> Result<Vec<RunSummary>, AppError> {
    crate::routines::svc_list_runs(routines, id)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
