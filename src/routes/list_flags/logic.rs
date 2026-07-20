//! Shared `list_flags` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::{Flag, RoutineStore};

/// List every open flag raised against routine `id`, oldest first.
pub fn build(store: &RoutineStore, id: &str) -> Result<Vec<Flag>, AppError> {
    crate::routines::svc_list_flags(store, id)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
