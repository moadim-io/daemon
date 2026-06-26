//! MCP server handler exposing cron-job tools over the Model Context Protocol.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::cron_jobs::{
    self, CreateRequest, CronStore, HandlerRegistry, ShutdownSignal, UpdateRequest,
};
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
    /// Notify handle that triggers a graceful server shutdown (the `shutdown` tool fires it,
    /// mirroring `POST /api/v1/shutdown` and `moadim stop`).
    shutdown: ShutdownSignal,
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
    /// New machines targeting list, or `None` to keep the existing value.
    machines: Option<Vec<String>>,
    /// New enabled state, or `None` to keep the existing value.
    enabled: Option<bool>,
}

/// Input for list tools that support local-machine filtering.
#[derive(Deserialize, JsonSchema)]
pub(super) struct LocalOnlyParam {
    /// When `true` (the default), only return entries targeting the current machine.
    /// Pass `false` to see entries from all machines.
    local_only: Option<bool>,
}

/// Input for the `lock_routines` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct LockRoutinesInput {
    /// Which sentinel to create: `"shared"` (committed `.lock`) or `"local"` (gitignored `.local.lock`).
    scope: String,
}

/// Input for the `unlock_routines` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct UnlockRoutinesInput {
    /// Which sentinel(s) to remove: `"shared"`, `"local"`, or `"all"` (both).
    scope: String,
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
    /// New machines targeting list, or `None` to keep the existing value.
    machines: Option<Vec<String>>,
    /// New enabled state, or `None` to keep the existing value.
    enabled: Option<bool>,
    /// New workbench TTL (seconds) for finished runs, or `None` to keep the existing value.
    ttl_secs: Option<u64>,
    /// New max runtime (seconds) for a single run before the watchdog kills it, or `None` to keep
    /// the existing value.
    max_runtime_secs: Option<u64>,
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
        shutdown: ShutdownSignal,
    ) -> Self {
        Self {
            store,
            handlers,
            routines,
            uptime_start,
            shutdown,
        }
    }

    /// Return server health status, uptime, build provenance, and filesystem locations.
    #[tool(description = "Get server health, uptime, build provenance, and filesystem locations")]
    fn health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let loc = crate::filesystem::FsLocation::current();
        let mut val = serde_json::json!({
            "status": "ok",
            // saturating_sub so a backward wall-clock adjustment can't underflow
            // (panic in debug, wrap to a huge value in release) — clamp to 0 instead.
            "uptime_secs": now_secs().saturating_sub(self.uptime_start),
            "running": true,
            // Build provenance, mirroring `GET /health` and `--version` so the
            // running build is identifiable consistently across all three surfaces.
            "version": crate::build_info::VERSION,
            "git_sha": crate::build_info::GIT_SHA,
            "build_date": crate::build_info::BUILD_DATE,
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

    /// Return managed cron jobs as a JSON array sorted by creation time.
    ///
    /// When `local_only` is `true` (the default), only jobs whose `machines` list includes the
    /// current machine are returned. Pass `false` to see all jobs regardless of machine.
    #[tool(description = "List managed cron jobs. Defaults to jobs targeting the current machine only; pass local_only=false to see all machines.")]
    fn list_cron_jobs(
        &self,
        Parameters(params): Parameters<LocalOnlyParam>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let query = cron_jobs::CronJobListQuery {
            local_only: Some(params.local_only.unwrap_or(true)),
        };
        Ok(ok(cron_jobs::svc_list(&self.store, &self.handlers, &query)))
    }

    /// Return the cron job matching the given UUID.
    #[tool(description = "Get a cron job by ID")]
    fn get_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_get(&self.store, &self.handlers, &id) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
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
                Err(error) => err(error),
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
            machines: input.machines,
            enabled: input.enabled,
        };
        Ok(
            match cron_jobs::svc_update(&self.store, &self.handlers, &input.id, req) {
                Ok(resp) => ok(resp),
                Err(error) => err(error),
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
                Err(error) => err(error),
            },
        )
    }

    /// Manually trigger a cron job immediately, recording the trigger time.
    #[tool(
        description = "Manually trigger a cron job outside its schedule, recording last_manual_trigger_at"
    )]
    fn trigger_cron_job(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_trigger(&self.store, &id) {
            Ok(job) => ok(job),
            Err(error) => err(error),
        })
    }

    /// Return managed routines as a JSON array sorted by creation time.
    ///
    /// When `local_only` is `true` (the default), only routines whose `machines` list includes the
    /// current machine are returned. Pass `false` to see all routines regardless of machine.
    #[tool(description = "List managed routines (agent-driven jobs). Defaults to routines targeting the current machine only; pass local_only=false to see all machines.")]
    fn list_routines(
        &self,
        Parameters(params): Parameters<LocalOnlyParam>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let query = routines::RoutineListQuery {
            local_only: Some(params.local_only.unwrap_or(true)),
            ..Default::default()
        };
        Ok(ok(routines::svc_list(&self.routines, &query)))
    }

    /// Return the routine matching the given UUID.
    #[tool(description = "Get a routine by ID")]
    fn get_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_get(&self.routines, &id) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
        })
    }

    /// Validate and persist a new routine, returning the created record.
    #[tool(
        description = "Create a new routine (agent-driven job). The `schedule` cron expression is interpreted in the local system timezone of the host running the daemon, NOT UTC. The response includes a `timezone` field and a `schedule_description` annotated with that timezone — verify them to confirm the firing time."
    )]
    fn create_routine(
        &self,
        Parameters(req): Parameters<CreateRoutineRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_create(&self.routines, req) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
        })
    }

    /// Apply provided fields to an existing routine, returning the updated record.
    #[tool(
        description = "Update fields of an existing routine. The `schedule` cron expression is interpreted in the local system timezone of the host running the daemon, NOT UTC. The response includes a `timezone` field and a `schedule_description` annotated with that timezone — verify them to confirm the firing time."
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
            machines: input.machines,
            enabled: input.enabled,
            ttl_secs: input.ttl_secs,
            max_runtime_secs: input.max_runtime_secs,
        };
        Ok(match routines::svc_update(&self.routines, &input.id, req) {
            Ok(resp) => ok(resp),
            Err(error) => err(error),
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
            Err(error) => err(error),
        })
    }

    /// Manually trigger a routine immediately, recording the trigger time.
    #[tool(
        description = "Manually trigger a routine outside its schedule, recording last_manual_trigger_at"
    )]
    fn trigger_routine(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_trigger(&self.routines, &id) {
            Ok(routine) => ok(routine),
            Err(error) => err(error),
        })
    }

    /// Reap finished, expired run workbenches immediately, returning how many were removed.
    #[tool(
        description = "Trigger cleanup of finished, expired routine run workbenches now instead of waiting for the hourly sweep. Returns the number of workbenches removed."
    )]
    fn cleanup_workbenches(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(routines::svc_cleanup(&self.routines)))
    }

    /// List the available agent registry keys a routine can launch.
    #[tool(description = "List the available agent registry keys a routine can launch")]
    fn list_agents(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(routines::available_agents()))
    }

    /// Return the contents of a cron job's log file, or an error if the job does not exist.
    #[tool(description = "Get a cron job's log file contents by ID")]
    fn cron_job_logs(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match cron_jobs::svc_logs(&self.store, &id) {
            Ok(logs) => ok(serde_json::json!({ "logs": logs })),
            Err(error) => err(error),
        })
    }

    /// Return the newest run log for a routine, or an error if the routine does not exist.
    #[tool(description = "Get a routine's newest run log by ID")]
    fn routine_logs(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_logs(&self.routines, &id) {
            Ok(logs) => ok(serde_json::json!({ "logs": logs })),
            Err(error) => err(error),
        })
    }

    /// Return whether the global routine lock is active and which sentinels are present.
    #[tool(
        description = "Get the global routine lock status. Returns `shared` (committed .lock file), `local` (gitignored .local.lock), and `locked` (either is present)."
    )]
    fn get_lock_status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(crate::global_lock::lock_status()))
    }

    /// Create a global lock sentinel that halts all routine scheduling and manual triggers without
    /// touching individual routine `enabled` states.
    #[tool(
        description = "Globally pause all routines by creating a lock sentinel. Use scope=\"shared\" for a committed .lock (shared via git) or scope=\"local\" for a gitignored .local.lock (machine-local). Individual routine enabled states are not modified."
    )]
    fn lock_routines(
        &self,
        Parameters(LockRoutinesInput { scope }): Parameters<LockRoutinesInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let lock_scope = match scope.as_str() {
            "shared" => crate::global_lock::LockScope::Shared,
            "local" => crate::global_lock::LockScope::Local,
            other => {
                return Ok(err(format!(
                    "unknown scope {other:?}; use \"shared\" or \"local\""
                )))
            }
        };
        if let Err(io_err) = crate::global_lock::set_lock(lock_scope, true) {
            return Ok(err(format!("failed to create lock sentinel: {io_err}")));
        }
        if let Err(sync_err) = crate::sync::routines::sync_routines_to_crontab(&self.routines) {
            log::warn!("crontab sync after lock failed: {sync_err}");
        }
        Ok(ok(crate::global_lock::lock_status()))
    }

    /// Remove a global lock sentinel, restoring scheduled and manual triggers for all enabled
    /// routines.
    #[tool(
        description = "Resume all routines by removing a lock sentinel. Use scope=\"shared\" to remove the committed .lock, scope=\"local\" to remove the gitignored .local.lock, or scope=\"all\" to remove both."
    )]
    fn unlock_routines(
        &self,
        Parameters(UnlockRoutinesInput { scope }): Parameters<UnlockRoutinesInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let scopes: Vec<crate::global_lock::LockScope> = match scope.as_str() {
            "shared" => vec![crate::global_lock::LockScope::Shared],
            "local" => vec![crate::global_lock::LockScope::Local],
            "all" => vec![
                crate::global_lock::LockScope::Shared,
                crate::global_lock::LockScope::Local,
            ],
            other => {
                return Ok(err(format!(
                    "unknown scope {other:?}; use \"shared\", \"local\", or \"all\""
                )))
            }
        };
        for scope_item in scopes {
            if let Err(io_err) = crate::global_lock::set_lock(scope_item, false) {
                return Ok(err(format!("failed to remove lock sentinel: {io_err}")));
            }
        }
        if let Err(sync_err) = crate::sync::routines::sync_routines_to_crontab(&self.routines) {
            log::warn!("crontab sync after unlock failed: {sync_err}");
        }
        Ok(ok(crate::global_lock::lock_status()))
    }

    /// Ask the server to stop gracefully, mirroring `POST /api/v1/shutdown` and `moadim stop`.
    #[tool(
        description = "Stop the running server gracefully. Mirrors the POST /api/v1/shutdown route and `moadim stop`."
    )]
    fn shutdown(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        log::info!("shutdown requested via MCP");
        self.shutdown.notify_one();
        Ok(ok(serde_json::json!({ "status": "shutting down" })))
    }

    /// Stop this server and start a fresh instance, mirroring `POST /api/v1/restart` and
    /// `moadim restart`. Delegates to a detached helper process that performs the swap.
    #[tool(
        description = "Restart the server: stop it and start a fresh instance. Mirrors the POST /api/v1/restart route and `moadim restart`."
    )]
    fn restart(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        log::info!("restart requested via MCP");
        Ok(crate::cli::spawn_restart().map_or_else(err, |helper_pid| {
            ok(serde_json::json!({ "status": "restarting", "helper_pid": helper_pid }))
        }))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
