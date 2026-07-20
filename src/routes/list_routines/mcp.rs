//! MCP `list_routines` tool — mirrors `GET /routines`, split into its own `#[tool_router]` block
//! so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::ListRoutinesParam;
use super::{ok, MoadimMcp};
use crate::routes::list_routines::logic::{self, RoutineListQuery};

#[tool_router(router = list_routines_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Return managed routines as a JSON array sorted by creation time.
    ///
    /// When `local_only` is `true` (the default), only routines whose `machines` list includes the
    /// current machine are returned. Pass `false` to see all routines regardless of machine.
    ///
    /// Prompts are omitted by default to keep the listing compact; pass `include_prompts=true`
    /// to include each routine's prompt.
    #[tool(
        description = "List managed routines (agent-driven jobs). Defaults to routines targeting the current machine only; pass local_only=false to see all machines. Prompts are omitted by default; pass include_prompts=true to include them."
    )]
    pub(super) fn list_routines(
        &self,
        Parameters(params): Parameters<ListRoutinesParam>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let query = RoutineListQuery {
            local_only: Some(params.local_only.unwrap_or(true)),
            include_prompts: Some(params.include_prompts.unwrap_or(false)),
            ..Default::default()
        };
        Ok(ok(logic::build(&self.routines, &self.routines_dir, &query)))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
