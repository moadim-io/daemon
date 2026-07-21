//! MCP `unlock_routines` tool — mirrors `DELETE /routines/lock`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::UnlockRoutinesInput;
use super::{err, ok, MoadimMcp};
use crate::routes::unlock_routines::logic;

#[tool_router(router = unlock_routines_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Remove a global lock sentinel, restoring scheduled and manual triggers for all enabled
    /// routines.
    #[tool(
        description = "Resume all routines by removing a lock sentinel. Use scope=\"shared\" to remove the committed .lock, scope=\"local\" to remove the gitignored .local.lock, or scope=\"all\" to remove both."
    )]
    pub(super) fn unlock_routines(
        &self,
        Parameters(UnlockRoutinesInput { scope }): Parameters<UnlockRoutinesInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &scope) {
            Ok(status) => ok(status),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
