//! MCP `move_routine` tool — mirrors `POST /routines/{id}/move`.

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::mcp_types::MoveRoutineInput;
use super::{err, ok, MoadimMcp};
use crate::routes::move_routine::logic::{self, MoveRoutineRequest};

#[tool_router(router = move_routine_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    /// Explicitly move a routine directory under the routines root.
    #[tool(
        description = "Move a routine to a filesystem folder/slug. The folder is not written to routine.toml; it is derived from the routine.toml location on disk."
    )]
    pub(super) fn move_routine(
        &self,
        Parameters(input): Parameters<MoveRoutineInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let req = MoveRoutineRequest {
            folder: input.folder,
            slug: input.slug,
        };
        Ok(match logic::build(&self.routines, &input.id, req) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
        })
    }
}
