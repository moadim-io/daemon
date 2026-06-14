//! MCP server handler exposing cron-job tools over the Model Context Protocol.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::cron_jobs::{self, CreateRequest, UpdateRequest};
use crate::utils::time::now_secs;

/// MCP server handler that exposes cron-job management as MCP tools.
#[derive(Clone)]
pub struct MoadimMcp {
    /// Unix timestamp (seconds) recorded at server startup.
    uptime_start: u64,
}

/// Input for the `echo` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    /// Message to echo back.
    message: String,
}

/// Input for tools that operate on a single job by ID.
#[derive(Deserialize, JsonSchema)]
struct IdInput {
    /// ID of the target cron job.
    id: String,
}

/// Input for the `update_cron_job` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct UpdateInput {
    /// ID of the cron job to update.
    id: String,
    /// New cron schedule, or `None` to keep existing.
    schedule: Option<String>,
    /// New command, or `None` to keep existing.
    command: Option<String>,
}

/// Wrap a serializable value in a successful `CallToolResult`.
fn ok(val: impl serde::Serialize) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string(&val).unwrap_or_default(),
    )])
}

/// Wrap an error message in a failed `CallToolResult`.
fn err(msg: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![Content::text(msg.to_string())])
}

#[tool_router(server_handler)]
impl MoadimMcp {
    /// Create a new `MoadimMcp` handler.
    pub fn new(uptime_start: u64) -> Self {
        Self { uptime_start }
    }

    /// Return server health status, uptime, and filesystem locations.
    #[tool(description = "Get server health, uptime, and filesystem locations")]
    fn health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let loc = crate::fs_location::FsLocation::current();
        let mut val = serde_json::json!({
            "status": "ok",
            "uptime_secs": now_secs() - self.uptime_start,
            "running": true,
        });
        if let (Some(obj), Ok(serde_json::Value::Object(loc_map))) =
            (val.as_object_mut(), serde_json::to_value(&loc))
        {
            obj.extend(loc_map);
        }
        Ok(ok(val))
    }

    /// Echo `message` back together with the current server timestamp.
    #[tool(description = "Echo a message back with a server timestamp")]
    fn echo(
        &self,
        Parameters(EchoInput { message }): Parameters<EchoInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(serde_json::json!({
            "message": message,
            "timestamp": now_secs(),
        })))
    }

    /// Return all cron jobs from the user crontab (managed and system entries).
    #[tool(description = "List all cron jobs from the user crontab")]
    fn list_cron_jobs(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_list() {
            Ok(jobs) => ok(jobs),
            Err(e) => err(e),
        })
    }

    /// Return the cron job matching the given ID.
    #[tool(description = "Get a cron job by ID")]
    fn get_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_get(&id) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }

    /// Add a new managed cron job to the user crontab.
    #[tool(description = "Add a new managed cron job to the user crontab")]
    fn create_cron_job(
        &self,
        Parameters(req): Parameters<CreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_create(req) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }

    /// Update schedule and/or command of an existing managed cron job.
    #[tool(description = "Update an existing managed cron job")]
    fn update_cron_job(
        &self,
        Parameters(input): Parameters<UpdateInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let req = UpdateRequest {
            schedule: input.schedule,
            command: input.command,
        };
        Ok(match cron_jobs::svc_update(&input.id, req) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }

    /// Remove a managed cron job from the user crontab.
    #[tool(description = "Remove a managed cron job from the user crontab")]
    fn delete_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_delete(&id) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }
}
