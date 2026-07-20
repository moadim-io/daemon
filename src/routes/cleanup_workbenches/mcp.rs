//! MCP `cleanup_workbenches` tool — mirrors `POST /routines/cleanup`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{ok, MoadimMcp};
use crate::routes::cleanup_workbenches::logic;

#[tool_router(router = cleanup_workbenches_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Reap finished, expired run workbenches immediately, returning how many were removed and
    /// the bytes freed.
    #[tool(
        description = "Trigger cleanup of finished, expired routine run workbenches now instead of waiting for the hourly sweep. Returns the number of workbenches removed and the total disk space freed in bytes."
    )]
    pub(super) fn cleanup_workbenches(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(logic::build(&self.routines)))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
