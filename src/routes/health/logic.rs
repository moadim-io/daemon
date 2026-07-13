//! Shared health-check logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::routines;
use crate::utils::time::now_secs;
use serde::Serialize;

/// External-binary dependencies the daemon relies on at runtime, and whether each is resolvable on
/// the daemon's `PATH`. Surfaced in [`HealthResponse`] so the UI/CLI can flag a missing dependency
/// instead of having routine runs silently no-op.
#[derive(Serialize, utoipa::ToSchema)]
pub struct DependencyHealth {
    /// Whether `tmux` (used to launch every routine agent) resolves on the daemon's `PATH`.
    pub tmux: bool,
    /// Whether `python3` resolves on the daemon's `PATH`. The built-in `claude` agent's `setup`
    /// step runs a `python3` snippet to pre-seed workspace-trust state; when it is missing that
    /// step fails silently and the routine still shows a healthy status (issue #404).
    pub python3: bool,
}

/// Response body for `GET /health`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Health status string (always `"ok"` when reachable).
    pub status: String,
    /// Seconds elapsed since the server started.
    pub uptime_secs: u64,
    /// Whether the server is running.
    pub running: bool,
    /// Resolved name of this machine (from `MOADIM_MACHINE`, `~/.config/moadim/machine.local.toml`, or hostname).
    pub machine: String,
    /// Presence of required external binaries on the daemon's `PATH`.
    pub dependencies: DependencyHealth,
    /// Daemon version (from `CARGO_PKG_VERSION`).
    pub version: String,
    /// Short git commit SHA the daemon was built from, or `"unknown"` outside a git checkout.
    pub git_sha: String,
    /// Committer date (`YYYY-MM-DD`) of the build commit, or `"unknown"` outside a git checkout.
    pub build_date: String,
    /// Absolute path of the server's working directory, or `None` if unresolvable.
    pub server_root: Option<String>,
    /// Absolute path of the directory containing the server executable, or `None` if unresolvable.
    pub server_exe_dir: Option<String>,
}

/// Build the current health snapshot for a server that started at `uptime_start` (unix seconds).
pub fn build(uptime_start: u64) -> HealthResponse {
    let loc = crate::filesystem::FsLocation::current();
    HealthResponse {
        status: "ok".to_string(),
        // saturating_sub so a backward wall-clock adjustment can't underflow
        // (panic in debug, wrap to a huge value in release) — clamp to 0 instead.
        uptime_secs: now_secs().saturating_sub(uptime_start),
        running: true,
        machine: crate::machine::current_machine(),
        dependencies: DependencyHealth {
            tmux: routines::tmux_available(),
            python3: routines::agent_command_available("python3"),
        },
        version: crate::build_info::VERSION.to_string(),
        git_sha: crate::build_info::GIT_SHA.to_string(),
        build_date: crate::build_info::BUILD_DATE.to_string(),
        server_root: loc.server_root,
        server_exe_dir: loc.server_exe_dir,
    }
}
