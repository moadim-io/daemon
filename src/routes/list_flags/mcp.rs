//! MCP `list_flags` tool — mirrors `GET /routines/{id}/flags`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::IdInput;
use super::{err, ok, MoadimMcp};
use crate::routes::list_flags::logic;

#[tool_router(router = list_flags_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// List every open flag raised against a routine.
    #[tool(description = "List open flags raised against a routine, oldest first")]
    pub(super) fn list_flags(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &id) {
            Ok(flags) => ok(flags),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
