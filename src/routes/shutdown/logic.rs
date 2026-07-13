//! Shared shutdown logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::routes::http::ShutdownSignal;
use serde::Serialize;

/// Response body for `POST /shutdown`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ShutdownResponse {
    /// Acknowledgement status (always `"shutting down"`).
    pub status: String,
}

/// Fire the shutdown signal and build the acknowledgement response.
pub fn build(shutdown: &ShutdownSignal) -> ShutdownResponse {
    shutdown.notify_one();
    ShutdownResponse {
        status: "shutting down".to_string(),
    }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
