//! MCP `create_routine` tool — mirrors `POST /routines`, split into its own `#[tool_router]`
//! block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::{err, ok, MoadimMcp};
use crate::routes::create_routine::logic;
use crate::routines::CreateRoutineRequest;

#[tool_router(router = create_routine_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Validate and persist a new routine, returning the created record.
    #[tool(
        description = "Create a new routine (agent-driven job). The `schedule` cron expression is interpreted in the local system timezone of the host running the daemon, NOT UTC. The response includes a `timezone` field and a `schedule_description` annotated with that timezone — verify them to confirm the firing time."
    )]
    pub(super) fn create_routine(
        &self,
        Parameters(req): Parameters<CreateRoutineRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, req) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
