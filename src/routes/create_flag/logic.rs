//! Shared `create_flag` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use serde::Deserialize;

use crate::error::AppError;
pub use crate::routines::{Flag, RoutineStore};

/// Request body for `POST /routines/{id}/flags`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateFlagRequest {
    /// Free-text flag category. Common examples: `"bug"`, `"gap"`, `"edge_case"`, `"question"`,
    /// `"blocker"` — any string is accepted.
    #[serde(rename = "type")]
    pub flag_type: String,
    /// Free-text description of what's unclear.
    pub description: String,
    /// `"general"` (committed, shared via git) or `"local"` (gitignored, machine-local).
    pub scope: String,
}

/// Raise a new flag against routine `id`, refreshing `prompt.compiled.local.md` so the next run's
/// prompt includes it.
pub fn build(
    store: &RoutineStore,
    id: &str,
    flag_type: &str,
    description: &str,
    scope: &str,
) -> Result<Flag, AppError> {
    crate::routines::svc_create_flag(store, id, flag_type, description, scope)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
