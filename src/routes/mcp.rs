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

/// The `move_routine` tool, kept in `routes/move_routine/mcp.rs` beside the
/// `POST /routines/{id}/move` HTTP handler it mirrors. Its own `#[tool_router]` block is combined
/// with this file's below.
#[path = "move_routine/mcp.rs"]
mod move_routine;

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
include!("preview_routine_prompt.rs");
