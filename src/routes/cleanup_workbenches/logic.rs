//! Shared `cleanup_workbenches` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

pub use crate::routines::CleanupResponse;
use crate::routines::RoutineStore;

/// Reap finished, expired run workbenches immediately, returning how many were removed and the
/// total disk space freed.
pub fn build(store: &RoutineStore) -> CleanupResponse {
    crate::routines::svc_cleanup(store)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
