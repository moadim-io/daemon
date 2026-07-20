//! MCP `delete_routine` tool — mirrors `DELETE /routines/{id}`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::IdInput;
use super::{err, ok, MoadimMcp};
use crate::routes::delete_routine::logic;

#[tool_router(router = delete_routine_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Remove the routine with the given UUID from the store.
    #[tool(description = "Delete a routine by ID")]
    pub(super) fn delete_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &id) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
