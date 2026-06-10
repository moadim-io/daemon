//! MCP server handler exposing cron-job tools over the Model Context Protocol.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::SystemTime;

use crate::cron_jobs::{self, CronStore, HandlerRegistry, CreateRequest, UpdateRequest};

/// MCP server handler that exposes cron-job management as MCP tools.
#[derive(Clone)]
pub struct MoadimMcp {
    /// Shared cron job store.
    store: CronStore,
    /// Registered handler identifiers used to annotate job responses.
    handlers: HandlerRegistry,
    /// Unix timestamp (seconds) recorded at server startup.
    uptime_start: u64,
    /// Generated tool router wiring method names to the MCP protocol.
    tool_router: ToolRouter<MoadimMcp>,
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
    /// UUID of the target cron job.
    id: String,
}

/// Schema override that marks `metadata` as a free-form JSON object.
fn metadata_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "object", "additionalProperties": true})
}

/// Input for the `update_cron_job` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct UpdateInput {
    /// UUID of the cron job to update.
    id: String,
    /// New cron expression, or `None` to keep the existing value.
    schedule: Option<String>,
    /// New handler identifier, or `None` to keep the existing value.
    handler: Option<String>,
    /// New metadata, or `None` to keep the existing value.
    #[schemars(schema_with = "metadata_schema")]
    metadata: Option<serde_json::Value>,
    /// New enabled state, or `None` to keep the existing value.
    enabled: Option<bool>,
}

/// Return current Unix time in whole seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
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
    /// Create a new `MoadimMcp` handler connected to the given store and handler registry.
    pub fn new(store: CronStore, handlers: HandlerRegistry, uptime_start: u64) -> Self {
        Self {
            store,
            handlers,
            uptime_start,
            tool_router: Self::tool_router(),
        }
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

    /// Return all managed cron jobs as a JSON array sorted by creation time.
    #[tool(description = "List all managed cron jobs")]
    fn list_cron_jobs(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(cron_jobs::svc_list(&self.store, &self.handlers)))
    }

    /// Return read-only system cron jobs discovered from crontab and `/etc/cron.d`.
    #[tool(description = "List read-only system cron jobs from crontab and /etc/cron.d (not managed by this server)")]
    fn list_system_cron_jobs(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(crate::system_cron::read_all()))
    }

    /// Return the cron job matching the given UUID.
    #[tool(description = "Get a cron job by ID")]
    fn get_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_get(&self.store, &self.handlers, &id) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Validate and persist a new cron job, returning the created record.
    #[tool(description = "Create a new cron job")]
    fn create_cron_job(
        &self,
        Parameters(req): Parameters<CreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_create(&self.store, &self.handlers, req) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Apply provided fields to an existing cron job, returning the updated record.
    #[tool(description = "Update fields of an existing cron job")]
    fn update_cron_job(
        &self,
        Parameters(input): Parameters<UpdateInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let req = UpdateRequest {
            schedule: input.schedule,
            handler: input.handler,
            metadata: input.metadata,
            enabled: input.enabled,
        };
        Ok(match cron_jobs::svc_update(&self.store, &self.handlers, &input.id, req) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Remove the cron job with the given UUID from the store.
    #[tool(description = "Delete a cron job by ID")]
    fn delete_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_delete(&self.store, &self.handlers, &id) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }
}
