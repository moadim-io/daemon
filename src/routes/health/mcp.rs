//! MCP `health` tool — mirrors `GET /health` (see `routes/health/mod.rs`), split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{ok, MoadimMcp};
use crate::routes::health::logic;

#[tool_router(router = health_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Return server health status, uptime, build provenance, and filesystem locations.
    #[tool(description = "Get server health, uptime, build provenance, and filesystem locations")]
    pub(super) fn health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(logic::build(self.uptime_start)))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
