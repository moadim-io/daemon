//! Shared restart logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use serde::Serialize;

/// Response body for `POST /restart` and the `restart` MCP tool.
#[derive(Serialize, utoipa::ToSchema)]
pub struct RestartResponse {
    /// Acknowledgement status (always `"restarting"`).
    pub status: String,
    /// PID of the detached helper process performing the stop-old-then-start-new restart.
    pub helper_pid: u32,
}

/// Spawn the detached restart helper and build the acknowledgement response.
pub fn build() -> anyhow::Result<RestartResponse> {
    let helper_pid = crate::cli::spawn_restart()?;
    Ok(RestartResponse {
        status: "restarting".to_string(),
        helper_pid,
    })
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
