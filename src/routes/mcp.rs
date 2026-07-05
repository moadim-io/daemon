//! MCP server handler exposing routine tools over the Model Context Protocol.

use crate::routes::http::ShutdownSignal;
use crate::routines::{self, CreateRoutineRequest, RoutineStore, UpdateRoutineRequest};
use crate::utils::time::now_secs;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

/// MCP server handler that exposes routine management as MCP tools.
#[derive(Clone)]
pub struct MoadimMcp {
    /// Shared routine store.
    routines: RoutineStore,
    /// Unix timestamp (seconds) recorded at server startup.
    uptime_start: u64,
    /// Notify handle that triggers a graceful server shutdown (the `shutdown` tool fires it,
    /// mirroring `POST /api/v1/shutdown` and `moadim stop`).
    shutdown: ShutdownSignal,
}

/// Input for tools that operate on a single routine by ID.
#[derive(Deserialize, JsonSchema)]
struct IdInput {
    /// UUID of the target routine.
    id: String,
}

/// Input for the `list_routines` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct ListRoutinesParam {
    /// When `true` (the default), only return routines targeting the current machine.
    /// Pass `false` to see routines from all machines.
    local_only: Option<bool>,
    /// When `true`, include each routine's `prompt` in the response. Defaults to `false` so listings stay compact; use `get_routine` to see a single routine's prompt.
    include_prompts: Option<bool>,
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

/// Input for the `create_flag` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct CreateFlagInput {
    /// UUID of the routine to flag.
    id: String,
    /// Free-text flag category. Common examples: "bug", "gap", `edge_case`, "question", "blocker"
    /// — any string is accepted.
    r#type: String,
    /// Free-text description of what's unclear.
    description: String,
    /// `"general"` (committed, shared via git) or `"local"` (gitignored, machine-local).
    scope: String,
}

/// Input for the `resolve_flag` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct ResolveFlagInput {
    /// UUID of the flagged routine.
    id: String,
    /// Flag filename, as returned by `create_flag`/`list_flags`.
    filename: String,
}

/// Input for the `snooze_routine` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct SnoozeRoutineInput {
    /// UUID of the routine to snooze.
    id: String,
    /// Unix timestamp (seconds) to skip scheduled fires until, or omit/null. Mutually exclusive
    /// with `skip_runs`.
    snoozed_until: Option<u64>,
    /// Number of upcoming scheduled fires to skip, or omit/null. Mutually exclusive with
    /// `snoozed_until`.
    skip_runs: Option<u32>,
}

/// Input for the `set_power_saving` MCP tool.
#[derive(Deserialize, JsonSchema)]
struct SetPowerSavingInput {
    /// UUID of the routine to update.
    id: String,
    /// `true` to pause scheduled and manual firing for power saving, `false` to resume.
    active: bool,
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
    /// New model ID, or `None` to keep the existing value. A blank/whitespace-only value clears
    /// the model back to the agent's own default.
    model: Option<String>,
    /// New prompt, or `None` to keep the existing value.
    prompt: Option<String>,
    /// New goal (a very short, ≤5-line statement of the routine's purpose), or `None` to keep the
    /// existing value. Send an empty string to clear it.
    goal: Option<String>,
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
    /// New tags list, or `None` to keep the existing value.
    tags: Option<Vec<String>>,
}

/// Wrap a serializable value in a successful `CallToolResult`.
fn ok(val: impl serde::Serialize) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(
        serde_json::to_string(&val).unwrap_or_default(),
    )])
}

/// Wrap an error message in a failed `CallToolResult`.
fn err(msg: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.to_string())])
}

#[tool_router(server_handler)]
impl MoadimMcp {
    /// Create a new `MoadimMcp` handler connected to the given routine store.
    pub fn new(routines: RoutineStore, uptime_start: u64, shutdown: ShutdownSignal) -> Self {
        Self {
            routines,
            uptime_start,
            shutdown,
        }
    }

    /// Return server health status, uptime, build provenance, and filesystem locations.
    #[tool(description = "Get server health, uptime, build provenance, and filesystem locations")]
    fn health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let loc = crate::filesystem::FsLocation::current();
        // Inline FsLocation fields directly so there are no conditional branches on serialization.
        let val = serde_json::json!({
            "status": "ok",
            // saturating_sub so a backward wall-clock adjustment can't underflow
            // (panic in debug, wrap to a huge value in release) — clamp to 0 instead.
            "uptime_secs": now_secs().saturating_sub(self.uptime_start),
            "running": true,
            // Resolved machine identity, mirroring `GET /health` and `GET /machine`.
            "machine": crate::machine::current_machine(),
            // Build provenance, mirroring `GET /health` and `--version` so the
            // running build is identifiable consistently across all three surfaces.
            "version": crate::build_info::VERSION,
            "git_sha": crate::build_info::GIT_SHA,
            "build_date": crate::build_info::BUILD_DATE,
            "server_root": loc.server_root,
            "server_exe_dir": loc.server_exe_dir,
        });
        Ok(ok(val))
    }

    /// Return managed routines as a JSON array sorted by creation time.
    ///
    /// When `local_only` is `true` (the default), only routines whose `machines` list includes the
    /// current machine are returned. Pass `false` to see all routines regardless of machine.
    ///
    /// Prompts are omitted by default to keep the listing compact; pass `include_prompts=true`
    /// to include each routine's prompt.
    #[tool(
        description = "List managed routines (agent-driven jobs). Defaults to routines targeting the current machine only; pass local_only=false to see all machines. Prompts are omitted by default; pass include_prompts=true to include them."
    )]
    fn list_routines(
        &self,
        Parameters(params): Parameters<ListRoutinesParam>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let query = routines::RoutineListQuery {
            local_only: Some(params.local_only.unwrap_or(true)),
            include_prompts: Some(params.include_prompts.unwrap_or(false)),
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
            model: input.model,
            prompt: input.prompt,
            goal: input.goal,
            repositories: input.repositories,
            machines: input.machines,
            enabled: input.enabled,
            ttl_secs: input.ttl_secs,
            max_runtime_secs: input.max_runtime_secs,
            tags: input.tags,
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

    /// Snooze a routine's scheduled fires without disabling it or touching manual triggers.
    #[tool(
        description = "Snooze a routine's scheduled (cron) fires without disabling it. Set snoozed_until (unix seconds) to skip fires until that time, or skip_runs (count) to skip that many upcoming scheduled fires — set exactly one, or neither to clear an active snooze. Manual triggers (trigger_routine) always bypass snooze and run normally."
    )]
    fn snooze_routine(
        &self,
        Parameters(SnoozeRoutineInput {
            id,
            snoozed_until,
            skip_runs,
        }): Parameters<SnoozeRoutineInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match routines::svc_snooze(&self.routines, &id, snoozed_until, skip_runs) {
                Ok(routine) => ok(routine),
                Err(error) => err(error),
            },
        )
    }

    /// Pause or resume a routine's scheduled and manual firing for power saving, without touching
    /// its `enabled` state or crontab line.
    #[tool(
        description = "Set or clear a routine's power-saving state. While active, both trigger_routine and the routine's cron schedule refuse to launch it (distinctly from a disabled routine) — its enabled toggle and crontab line are untouched, so it resumes firing on its own once cleared."
    )]
    fn set_power_saving(
        &self,
        Parameters(SetPowerSavingInput { id, active }): Parameters<SetPowerSavingInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match routines::svc_set_power_saving(&self.routines, &id, active) {
                Ok(routine) => ok(routine),
                Err(error) => err(error),
            },
        )
    }

    /// Reap finished, expired run workbenches immediately, returning how many were removed and the
    /// bytes freed.
    #[tool(
        description = "Trigger cleanup of finished, expired routine run workbenches now instead of waiting for the hourly sweep. Returns the number of workbenches removed and the total disk space freed in bytes."
    )]
    fn cleanup_workbenches(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(routines::svc_cleanup(&self.routines)))
    }

    /// List the available agent registry keys a routine can launch.
    #[tool(description = "List the available agent registry keys a routine can launch")]
    fn list_agents(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(routines::available_agents()))
    }

    /// Raise a new flag against a routine, refreshing its `prompt.compiled.md` so the next run's
    /// "Open flags" section includes it.
    #[tool(
        description = "Flag something unclear about a routine mid-run — a gap, bug, edge case, or question the agent hit with no other channel to surface it (the run happens unattended inside tmux). `type` is free text (common examples: \"bug\", \"gap\", \"edge_case\", \"question\", \"blocker\"); `scope` is \"general\" (committed, shared via git) or \"local\" (gitignored, machine-local). Unresolved flags are shown back to the agent in the routine's prompt on its next run."
    )]
    fn create_flag(
        &self,
        Parameters(CreateFlagInput {
            id,
            r#type,
            description,
            scope,
        }): Parameters<CreateFlagInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match routines::svc_create_flag(&self.routines, &id, &r#type, &description, &scope) {
                Ok(flag) => ok(flag),
                Err(error) => err(error),
            },
        )
    }

    /// List every open flag raised against a routine.
    #[tool(description = "List open flags raised against a routine, oldest first")]
    fn list_flags(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_list_flags(&self.routines, &id) {
            Ok(flags) => ok(flags),
            Err(error) => err(error),
        })
    }

    /// Resolve (delete) a flag by filename, refreshing `prompt.compiled.md` so it stops appearing
    /// in the next run's prompt.
    #[tool(
        description = "Resolve a routine flag by filename (as returned by create_flag/list_flags), removing it"
    )]
    fn resolve_flag(
        &self,
        Parameters(ResolveFlagInput { id, filename }): Parameters<ResolveFlagInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match routines::svc_resolve_flag(&self.routines, &id, &filename) {
                Ok(()) => ok(serde_json::json!({ "status": "resolved" })),
                Err(error) => err(error),
            },
        )
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

    /// List a routine's runs (live workbenches plus durable history), newest first.
    #[tool(
        description = "List a routine's runs, newest first — each run's workbench id (pass to the REST endpoint GET /routines/{id}/runs/{workbench}/log to fetch its log), start/finish time, status, and exit code"
    )]
    fn list_routine_runs(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_list_runs(&self.routines, &id) {
            Ok(runs) => ok(runs),
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
#[path = "mcp_lock_tests.rs"]
mod mcp_lock_tests;
#[cfg(test)]
#[path = "mcp_parity_tests.rs"]
mod mcp_parity_tests;
#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
