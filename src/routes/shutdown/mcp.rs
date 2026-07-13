//! MCP `shutdown` tool — mirrors `POST /shutdown`, split into its own `#[tool_router]`
//! block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{ok, MoadimMcp};
use crate::routes::shutdown::logic;

#[tool_router(router = shutdown_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Ask the server to stop gracefully, mirroring `POST /api/v1/shutdown` and `moadim stop`.
    #[tool(
        description = "Stop the running server gracefully. Mirrors the POST /api/v1/shutdown route and `moadim stop`."
    )]
    pub(super) fn shutdown(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        log::info!("shutdown requested via MCP");
        Ok(ok(logic::build(&self.shutdown)))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
