//! MCP `update_routine` tool — mirrors `PATCH /routines/{id}`, split into its own
//! `#[tool_router]` block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::UpdateRoutineInput;
use super::{err, ok, MoadimMcp};
use crate::routes::update_routine::logic::{self, UpdateRoutineRequest};

#[tool_router(router = update_routine_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Apply provided fields to an existing routine, returning the updated record.
    #[tool(
        description = "Update fields of an existing routine. The `schedule` cron expression is interpreted in the local system timezone of the host running the daemon, NOT UTC. The response includes a `timezone` field and a `schedule_description` annotated with that timezone — verify them to confirm the firing time."
    )]
    pub(super) fn update_routine(
        &self,
        Parameters(input): Parameters<UpdateRoutineInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let req = UpdateRoutineRequest {
            schedule: input.schedule,
            title: input.title,
            agent: input.agent,
            model: input.model,
            prompt: input.prompt,
            goal: input.goal,
            repositories: input.repositories,
            machines: input.machines,
            enabled: input.enabled,
            ttl_secs: input.ttl_secs,
            max_runtime_secs: input.max_runtime_secs,
            tags: input.tags,
            env: input.env,
        };
        Ok(match logic::build(&self.routines, &input.id, req) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
