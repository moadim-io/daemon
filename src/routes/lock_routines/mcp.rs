//! MCP `lock_routines` tool — mirrors `POST /routines/lock`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::LockRoutinesInput;
use super::{err, ok, MoadimMcp};
use crate::routes::lock_routines::logic;

#[tool_router(router = lock_routines_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Create a global lock sentinel that halts all routine scheduling and manual triggers
    /// without touching individual routine `enabled` states.
    #[tool(
        description = "Globally pause all routines by creating a lock sentinel. Use scope=\"shared\" for a committed .lock (shared via git) or scope=\"local\" for a gitignored .local.lock (machine-local). Individual routine enabled states are not modified."
    )]
    pub(super) fn lock_routines(
        &self,
        Parameters(LockRoutinesInput { scope }): Parameters<LockRoutinesInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match logic::build(&self.routines, &scope) {
            Ok(status) => ok(status),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
