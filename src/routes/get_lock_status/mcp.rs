//! MCP `get_lock_status` tool — mirrors `GET /routines/lock`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{ok, MoadimMcp};
use crate::routes::get_lock_status::logic;

#[tool_router(router = get_lock_status_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Report whether the shared and/or local lock sentinels are present, mirroring
    /// `GET /api/v1/routines/lock`.
    #[tool(
        description = "Get the global routine lock status. Returns `shared` (committed .lock file), `local` (gitignored .local.lock), and `locked` (either is present)."
    )]
    #[allow(
        clippy::unused_self,
        reason = "tool_router dispatches every handler through self.method(...) uniformly"
    )]
    pub(super) fn get_lock_status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(logic::build()))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
