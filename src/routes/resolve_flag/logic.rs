//! Shared `resolve_flag` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub use crate::routines::RoutineStore;

/// Resolve (delete) flag `filename` on routine `id`, refreshing `prompt.compiled.local.md` so it
/// stops appearing in the next run's prompt.
pub fn build(store: &RoutineStore, id: &str, filename: &str) -> Result<(), AppError> {
    crate::routines::svc_resolve_flag(store, id, filename)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
