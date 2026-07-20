//! MCP server handler exposing routine tools over the Model Context Protocol.

use crate::routes::http::ShutdownSignal;
use crate::routines::{self, RoutineStore};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

#[path = "mcp_types.rs"]
mod mcp_types;
use mcp_types::{IdInput, SetPowerSavingInput, SnoozeRoutineInput};

/// The `health` tool, kept in `routes/health/mcp.rs` beside the `GET /health` HTTP handler it
/// mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "health/mcp.rs"]
mod health;

/// The `shutdown` tool, kept in `routes/shutdown/mcp.rs` beside the `POST /shutdown` HTTP handler
/// it mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "shutdown/mcp.rs"]
mod shutdown;

/// The `restart` tool, kept in `routes/restart/mcp.rs` beside the `POST /restart` HTTP handler it
/// mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "restart/mcp.rs"]
mod restart;

/// The `get_lock_status` tool, kept in `routes/get_lock_status/mcp.rs` beside the
/// `GET /routines/lock` HTTP handler it mirrors. Its own `#[tool_router]` block is combined with
/// this file's below.
#[path = "get_lock_status/mcp.rs"]
mod get_lock_status;

/// The `list_agents` tool, kept in `routes/list_agents/mcp.rs` beside the `GET /agents` HTTP
/// handler it mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "list_agents/mcp.rs"]
mod list_agents;

/// The `cleanup_workbenches` tool, kept in `routes/cleanup_workbenches/mcp.rs` beside the
/// `POST /routines/cleanup` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "cleanup_workbenches/mcp.rs"]
mod cleanup_workbenches;

/// The `list_routines` tool, kept in `routes/list_routines/mcp.rs` beside the `GET /routines`
/// HTTP handler it mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "list_routines/mcp.rs"]
mod list_routines;

/// The `get_routine` tool, kept in `routes/get_routine/mcp.rs` beside the `GET /routines/{id}`
/// HTTP handler it mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "get_routine/mcp.rs"]
mod get_routine;

/// The `delete_routine` tool, kept in `routes/delete_routine/mcp.rs` beside the
/// `DELETE /routines/{id}` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "delete_routine/mcp.rs"]
mod delete_routine;

/// The `create_routine` tool, kept in `routes/create_routine/mcp.rs` beside the `POST /routines`
/// HTTP handler it mirrors. Its own `#[tool_router]` block is combined with this file's below.
#[path = "create_routine/mcp.rs"]
mod create_routine;

/// The `list_routine_runs` tool, kept in `routes/list_routine_runs/mcp.rs` beside the
/// `GET /routines/{id}/runs` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "list_routine_runs/mcp.rs"]
mod list_routine_runs;

/// The `update_routine` tool, kept in `routes/update_routine/mcp.rs` beside the
/// `PATCH /routines/{id}` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "update_routine/mcp.rs"]
mod update_routine;

/// The `trigger_routine` tool, kept in `routes/trigger_routine/mcp.rs` beside the
/// `POST /routines/{id}/trigger` HTTP handler it mirrors. Its own `#[tool_router]` block is
/// combined with this file's below.
#[path = "trigger_routine/mcp.rs"]
mod trigger_routine;

/// The `create_flag` tool, kept in `routes/create_flag/mcp.rs` beside the
/// `POST /routines/{id}/flags` HTTP handler it mirrors. Its own `#[tool_router]` block is
/// combined with this file's below.
#[path = "create_flag/mcp.rs"]
mod create_flag;

/// The `list_flags` tool, kept in `routes/list_flags/mcp.rs` beside the
/// `GET /routines/{id}/flags` HTTP handler it mirrors. Its own `#[tool_router]` block is
/// combined with this file's below.
#[path = "list_flags/mcp.rs"]
mod list_flags;

/// The `resolve_flag` tool, kept in `routes/resolve_flag/mcp.rs` beside the
/// `DELETE /routines/{id}/flags/{filename}` HTTP handler it mirrors. Its own `#[tool_router]`
/// block is combined with this file's below.
#[path = "resolve_flag/mcp.rs"]
mod resolve_flag;

/// The `lock_routines` tool, kept in `routes/lock_routines/mcp.rs` beside the
/// `POST /routines/lock` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "lock_routines/mcp.rs"]
mod lock_routines;

/// The `unlock_routines` tool, kept in `routes/unlock_routines/mcp.rs` beside the
/// `DELETE /routines/lock` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "unlock_routines/mcp.rs"]
mod unlock_routines;

/// MCP server handler that exposes routine management as MCP tools.
#[derive(Clone)]
pub struct MoadimMcp {
    /// Shared routine store.
    routines: RoutineStore,
    /// On-disk directory the routine store is re-scanned from on every list/get tool call.
    /// Defaults to [`crate::paths::routines_dir`]; tests point it at a tempdir for isolation.
    routines_dir: std::path::PathBuf,
    /// Unix timestamp (seconds) recorded at server startup.
    uptime_start: u64,
    /// Notify handle that triggers a graceful server shutdown (the `shutdown` tool fires it,
    /// mirroring `POST /api/v1/shutdown` and `moadim stop`).
    shutdown: ShutdownSignal,
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

#[tool_router]
impl MoadimMcp {
    /// Create a new `MoadimMcp` handler connected to the given routine store.
    pub fn new(
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
        description = "Preview the exact composed prompt body a routine's run would receive, without triggering a real run (no workbench, no agent launch). Does not include the routine-origin disclosure written separately to CLAUDE.md at trigger time."
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
/// [`list_flags`], [`resolve_flag`], [`lock_routines`], and [`unlock_routines`] modules), since a
/// `#[tool_router]` block only collects the `#[tool]` methods in its own `impl`.
#[tool_handler(router = (Self::tool_router() + Self::health_tool_router() + Self::shutdown_tool_router() + Self::restart_tool_router() + Self::get_lock_status_tool_router() + Self::list_agents_tool_router() + Self::cleanup_workbenches_tool_router() + Self::list_routines_tool_router() + Self::get_routine_tool_router() + Self::delete_routine_tool_router() + Self::create_routine_tool_router() + Self::list_routine_runs_tool_router() + Self::update_routine_tool_router() + Self::trigger_routine_tool_router() + Self::create_flag_tool_router() + Self::list_flags_tool_router() + Self::resolve_flag_tool_router() + Self::lock_routines_tool_router() + Self::unlock_routines_tool_router()))]
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
