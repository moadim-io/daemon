//! Shared `list_routines` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

pub use crate::routines::{RoutineListQuery, RoutineResponse, RoutineStore};

/// Build the current routine listing for the given query, reloading the store from disk first.
pub fn build(
    routines: &RoutineStore,
    routines_dir: &std::path::Path,
    query: &RoutineListQuery,
) -> Vec<RoutineResponse> {
    crate::routines::svc_list(routines, routines_dir, query)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
