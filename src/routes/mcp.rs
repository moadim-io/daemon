//! MCP server handler exposing cron-job tools over the Model Context Protocol.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::cron_jobs::{self, CreateRequest, CronStore, HandlerRegistry, UpdateRequest};
use crate::routines::{self, CreateRoutineRequest, RoutineStore, UpdateRoutineRequest};
use crate::utils::time::now_secs;

/// MCP server handler that exposes cron-job and routine management as MCP tools.
#[derive(Clone)]
pub struct MoadimMcp {
    /// Shared cron job store.
    store: CronStore,
    /// Registered handler identifiers used to annotate job responses.
    handlers: HandlerRegistry,
    /// Shared routine store.
    routines: RoutineStore,
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
    /// UUID of the target cron job.
    id: String,
}

/// Input for the `update_cron_job` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct UpdateInput {
    /// UUID of the cron job to update.
    id: String,
    /// New cron expression, or `None` to keep the existing value. Evaluated in the
    /// host's local system timezone (the OS crontab timezone), not UTC.
    schedule: Option<String>,
    /// New handler identifier, or `None` to keep the existing value.
    handler: Option<String>,
    /// New metadata, or `None` to keep the existing value.
    #[schemars(schema_with = "crate::utils::schema::metadata_schema")]
    metadata: Option<serde_json::Value>,
    /// New enabled state, or `None` to keep the existing value.
    enabled: Option<bool>,
}

/// Input for the `update_routine` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct UpdateRoutineInput {
    /// UUID of the routine to update.
    id: String,
    /// New cron expression, or `None` to keep the existing value. Evaluated in the
    /// host's local system timezone (the OS crontab timezone), not UTC.
    schedule: Option<String>,
    /// New title, or `None` to keep the existing value.
    title: Option<String>,
    /// New agent key, or `None` to keep the existing value.
    agent: Option<String>,
    /// New prompt, or `None` to keep the existing value.
    prompt: Option<String>,
    /// New repositories list, or `None` to keep the existing value.
    repositories: Option<Vec<crate::routines::Repository>>,
    /// New enabled state, or `None` to keep the existing value.
    enabled: Option<bool>,
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
    /// Create a new `MoadimMcp` handler connected to the given stores and handler registry.
    pub fn new(
        store: CronStore,
        handlers: HandlerRegistry,
        routines: RoutineStore,
        uptime_start: u64,
    ) -> Self {
        Self {
            store,
            handlers,
            routines,
            uptime_start,
        }
    }

    /// Return server health status, uptime, and filesystem locations.
    #[tool(description = "Get server health, uptime, and filesystem locations")]
    fn health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let loc = crate::filesystem::FsLocation::current();
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
    #[tool(
        description = "Create a new cron job. The schedule cron expression is evaluated in the host's local system timezone (the OS crontab timezone), not UTC."
    )]
    fn create_cron_job(
        &self,
        Parameters(req): Parameters<CreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match cron_jobs::svc_create(&self.store, &self.handlers, req) {
                Ok(resp) => ok(resp),
                Err(e) => err(e),
            },
        )
    }

    /// Apply provided fields to an existing cron job, returning the updated record.
    #[tool(
        description = "Update fields of an existing cron job. A new schedule cron expression is evaluated in the host's local system timezone (the OS crontab timezone), not UTC."
    )]
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
        Ok(
            match cron_jobs::svc_update(&self.store, &self.handlers, &input.id, req) {
                Ok(resp) => ok(resp),
                Err(e) => err(e),
            },
        )
    }

    /// Remove the cron job with the given UUID from the store.
    #[tool(description = "Delete a cron job by ID")]
    fn delete_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match cron_jobs::svc_delete(&self.store, &self.handlers, &id) {
                Ok(resp) => ok(resp),
                Err(e) => err(e),
            },
        )
    }

    /// Manually trigger a cron job immediately, recording the trigger time.
    #[tool(
        description = "Manually trigger a cron job outside its schedule, recording last_triggered_at"
    )]
    fn trigger_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_trigger(&self.store, &id) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }

    /// Return all managed routines as a JSON array sorted by creation time.
    #[tool(description = "List all managed routines (agent-driven jobs)")]
    fn list_routines(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(routines::svc_list(&self.routines)))
    }

    /// Return the routine matching the given UUID.
    #[tool(description = "Get a routine by ID")]
    fn get_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_get(&self.routines, &id) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Validate and persist a new routine, returning the created record.
    #[tool(
        description = "Create a new routine (agent-driven job). The schedule cron expression is evaluated in the host's local system timezone (the OS crontab timezone), not UTC."
    )]
    fn create_routine(
        &self,
        Parameters(req): Parameters<CreateRoutineRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_create(&self.routines, req) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Apply provided fields to an existing routine, returning the updated record.
    #[tool(
        description = "Update fields of an existing routine. A new schedule cron expression is evaluated in the host's local system timezone (the OS crontab timezone), not UTC."
    )]
    fn update_routine(
        &self,
        Parameters(input): Parameters<UpdateRoutineInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let req = UpdateRoutineRequest {
            schedule: input.schedule,
            title: input.title,
            agent: input.agent,
            prompt: input.prompt,
            repositories: input.repositories,
            enabled: input.enabled,
        };
        Ok(match routines::svc_update(&self.routines, &input.id, req) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Remove the routine with the given UUID from the store.
    #[tool(description = "Delete a routine by ID")]
    fn delete_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_delete(&self.routines, &id) {
            Ok(resp) => ok(resp),
            Err(e) => err(e),
        })
    }

    /// Manually trigger a routine immediately, recording the trigger time.
    #[tool(
        description = "Manually trigger a routine outside its schedule, recording last_triggered_at"
    )]
    fn trigger_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_trigger(&self.routines, &id) {
            Ok(routine) => ok(routine),
            Err(e) => err(e),
        })
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
