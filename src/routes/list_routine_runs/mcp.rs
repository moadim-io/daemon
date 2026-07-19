//! MCP `list_routine_runs` tool — mirrors `GET /routines/{id}/runs`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::IdInput;
use super::{err, ok, MoadimMcp};
use crate::routes::list_routine_runs::logic;

#[tool_router(router = list_routine_runs_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// List a routine's runs (live workbenches plus durable history), newest first.
    #[tool(
        description = "List a routine's runs, newest first — each run's workbench id (pass to the REST endpoints GET /routines/{id}/runs/{workbench}/log for its log or GET /routines/{id}/runs/{workbench}/summary for the agent's work summary), start/finish time, status, and exit code"
    )]
    pub(super) fn list_routine_runs(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &id) {
            Ok(runs) => ok(runs),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
