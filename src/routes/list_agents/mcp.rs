//! MCP `list_agents` tool — mirrors `GET /agents`, split into its own `#[tool_router]` block so
//! it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{ok, MoadimMcp};
use crate::routes::list_agents::logic;

#[tool_router(router = list_agents_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// List the available agent registry keys a routine can launch.
    #[tool(description = "List the available agent registry keys a routine can launch")]
    #[allow(
        clippy::unused_self,
        reason = "tool_router dispatches every handler through self.method(...) uniformly"
    )]
    pub(super) fn list_agents(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(logic::build()))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
