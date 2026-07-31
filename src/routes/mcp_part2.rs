
#[tool_router]
impl MoadimMcp {
    /// Create a new `MoadimMcp` handler connected to the given routine store.
    pub const fn new(
        routines: RoutineStore,
        routines_dir: std::path::PathBuf,
        uptime_start: u64,
        shutdown: ShutdownSignal,
    ) -> Self {
        Self {
            routines,
            routines_dir,
            uptime_start,
            shutdown,
        }
    }

    /// Return the exact prompt body a routine's run would receive, without creating a workbench
    /// or launching an agent.
    #[tool(
        description = "Preview the exact composed prompt body a routine's run would receive, without triggering a real run (no workbench, no agent launch). Includes the routine-origin disclosure because it is part of the composed prompt."
    )]
    fn preview_routine_prompt(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(
            match routines::svc_get_prompt_preview(&self.routines, &id) {
                Ok(prompt) => ok(serde_json::json!({ "prompt": prompt })),
                Err(error) => err(error),
            },
        )
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

    /// Return the newest run log for a routine, or an error if the routine does not exist.
    #[tool(description = "Get a routine's newest run log by ID")]
    fn routine_logs(
        &self,
        Parameters(IdInput { id }): Parameters<IdInput>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(match routines::svc_logs(&self.routines, &id) {
            Ok(logs) => ok(serde_json::json!({
                "logs": logs.content,
                "total_bytes": logs.total_bytes,
                "truncated": logs.truncated,
            })),
            Err(error) => err(error),
        })
    }
}

/// Combines this file's tool router with the split-out tools' (see the [`health`], [`shutdown`],
/// [`restart`], [`get_lock_status`], [`list_agents`], [`cleanup_workbenches`],
/// [`list_routines`], [`get_routine`], [`delete_routine`], [`create_routine`],
/// [`list_routine_runs`], [`update_routine`], [`trigger_routine`], [`create_flag`],
/// [`list_flags`], [`resolve_flag`], [`move_routine`], [`lock_routines`], and [`unlock_routines`] modules), since a
/// `#[tool_router]` block only collects the `#[tool]` methods in its own `impl`.
#[tool_handler(router = (Self::tool_router() + Self::health_tool_router() + Self::shutdown_tool_router() + Self::restart_tool_router() + Self::get_lock_status_tool_router() + Self::list_agents_tool_router() + Self::cleanup_workbenches_tool_router() + Self::list_routines_tool_router() + Self::get_routine_tool_router() + Self::delete_routine_tool_router() + Self::create_routine_tool_router() + Self::list_routine_runs_tool_router() + Self::update_routine_tool_router() + Self::move_routine_tool_router() + Self::trigger_routine_tool_router() + Self::create_flag_tool_router() + Self::list_flags_tool_router() + Self::resolve_flag_tool_router() + Self::lock_routines_tool_router() + Self::unlock_routines_tool_router()))]
impl rmcp::ServerHandler for MoadimMcp {}

#[cfg(test)]
#[path = "mcp_parity_tests.rs"]
mod mcp_parity_tests;
#[cfg(test)]
#[path = "mcp_prompt_preview_tests.rs"]
mod mcp_prompt_preview_tests;
#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
