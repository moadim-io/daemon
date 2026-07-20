//! MCP `create_flag` tool — mirrors `POST /routines/{id}/flags`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::CreateFlagInput;
use super::{err, ok, MoadimMcp};
use crate::routes::create_flag::logic;

#[tool_router(router = create_flag_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Raise a new flag against a routine, refreshing its `prompt.compiled.local.md` so the next run's
    /// "Open flags" section includes it.
    #[tool(
        description = "Flag something unclear about a routine mid-run — a gap, bug, edge case, or question the agent hit with no other channel to surface it (the run happens unattended inside tmux). `type` is free text (common examples: \"bug\", \"gap\", \"edge_case\", \"question\", \"blocker\"); `scope` is \"general\" (committed, shared via git) or \"local\" (gitignored, machine-local). Unresolved flags are shown back to the agent in the routine's prompt on its next run."
    )]
    pub(super) fn create_flag(
        &self,
        Parameters(CreateFlagInput {
            id,
            r#type,
            description,
            scope,
        }): Parameters<CreateFlagInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match logic::build(&self.routines, &id, &r#type, &description, &scope) {
                Ok(flag) => ok(flag),
                Err(error) => err(error),
            },
        )
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
