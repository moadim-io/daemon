//! MCP `restart` tool — mirrors `POST /restart`, split into its own `#[tool_router]`
//! block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{err, ok, MoadimMcp};
use crate::routes::restart::logic;

#[tool_router(router = restart_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Stop this server and start a fresh instance, mirroring `POST /api/v1/restart` and
    /// `moadim restart`. Delegates to a detached helper process that performs the swap.
    #[tool(
        description = "Restart the server: stop it and start a fresh instance. Mirrors the POST /api/v1/restart route and `moadim restart`."
    )]
    #[allow(
        clippy::unused_self,
        reason = "tool_router dispatches every handler through self.method(...) uniformly"
    )]
    pub(super) fn restart(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        log::info!("restart requested via MCP");
        Ok(logic::build().map_or_else(err, ok))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
