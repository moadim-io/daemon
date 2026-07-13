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
        let loc = crate::filesystem::FsLocation::current();
        // Same fields as `GET /health`, plus the two filesystem locations below — build on the
        // shared logic rather than re-deriving status/uptime/dependencies/version by hand.
        let mut val = serde_json::to_value(logic::build(self.uptime_start)).unwrap_or_default();
        if let serde_json::Value::Object(ref mut map) = val {
            map.insert(
                "server_root".to_string(),
                serde_json::json!(loc.server_root),
            );
            map.insert(
                "server_exe_dir".to_string(),
                serde_json::json!(loc.server_exe_dir),
            );
        }
        Ok(ok(val))
    }
}
