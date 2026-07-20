//! MCP `resolve_flag` tool — mirrors `DELETE /routines/{id}/flags/{filename}`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::ResolveFlagInput;
use super::{err, ok, MoadimMcp};
use crate::routes::resolve_flag::logic;

#[tool_router(router = resolve_flag_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Resolve (delete) a flag by filename, refreshing `prompt.compiled.local.md` so it stops
    /// appearing in the next run's prompt.
    #[tool(
        description = "Resolve a routine flag by filename (as returned by create_flag/list_flags), removing it"
    )]
    pub(super) fn resolve_flag(
        &self,
        Parameters(ResolveFlagInput { id, filename }): Parameters<ResolveFlagInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &id, &filename) {
            Ok(()) => ok(serde_json::json!({ "status": "resolved" })),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
