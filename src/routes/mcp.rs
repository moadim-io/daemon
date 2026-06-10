use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::SystemTime;

use crate::cron_jobs::{self, CronJobResponse, CronStore, HandlerRegistry, CreateRequest, UpdateRequest};

#[derive(Clone)]
pub struct MoadimMcp {
    store: CronStore,
    handlers: HandlerRegistry,
    uptime_start: u64,
    tool_router: ToolRouter<MoadimMcp>,
}

#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    message: String,
}

#[derive(Deserialize, JsonSchema)]
struct IdInput {
    id: String,
}

fn metadata_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "object", "additionalProperties": true})
}

#[derive(Deserialize, JsonSchema)]
struct UpdateInput {
    id: String,
    schedule: Option<String>,
    handler: Option<String>,
    #[schemars(schema_with = "metadata_schema")]
    metadata: Option<serde_json::Value>,
    enabled: Option<bool>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn ok(val: impl serde::Serialize) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string(&val).unwrap_or_default(),
    )])
}

fn err(msg: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![Content::text(msg.to_string())])
}

#[tool_router(server_handler)]
impl MoadimMcp {
    pub fn new(store: CronStore, handlers: HandlerRegistry, uptime_start: u64) -> Self {
        Self {
            store,
            handlers,
            uptime_start,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get server health and uptime")]
    fn health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(serde_json::json!({
            "status": "ok",
            "uptime_secs": now_secs() - self.uptime_start,
            "running": true,
        })))
    }

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

    #[tool(description = "List all managed cron jobs")]
    fn list_cron_jobs(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let jobs: Vec<CronJobResponse> = cron_jobs::svc_list(&self.store)
            .into_iter()
            .map(|j| CronJobResponse::from_job(j, &self.handlers))
            .collect();
        Ok(ok(jobs))
    }

    #[tool(description = "List read-only system cron jobs from crontab and /etc/cron.d (not managed by this server)")]
    fn list_system_cron_jobs(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(crate::system_cron::read_all()))
    }

    #[tool(description = "Get a cron job by ID")]
    fn get_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_get(&self.store, &id) {
            Ok(job) => ok(CronJobResponse::from_job(job, &self.handlers)),
            Err(e) => err(e),
        })
    }

    #[tool(description = "Create a new cron job")]
    fn create_cron_job(
        &self,
        Parameters(req): Parameters<CreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_create(&self.store, req) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }

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
        Ok(match cron_jobs::svc_update(&self.store, &input.id, req) {
            Ok(job) => ok(job),
            Err(e) => err(e),
        })
    }

    #[tool(description = "Delete a cron job by ID")]
    fn delete_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_delete(&self.store, &id) {
            Ok(()) => ok(serde_json::json!({"deleted": id})),
            Err(e) => err(e),
        })
    }
}
