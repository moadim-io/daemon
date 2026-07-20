//! MCP `trigger_routine` tool — mirrors `POST /routines/{id}/trigger`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::IdInput;
use super::{err, ok, MoadimMcp};
use crate::routes::trigger_routine::logic;

#[tool_router(router = trigger_routine_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Manually trigger a routine immediately, recording `last_manual_trigger_at`.
    #[tool(
        description = "Manually trigger a routine outside its schedule, recording last_manual_trigger_at"
    )]
    pub(super) fn trigger_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &id) {
            Ok(routine) => ok(routine),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
