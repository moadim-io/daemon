//! Shared `get_lock_status` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

pub use crate::global_lock::LockStatus;

/// Build the current global lock status payload.
pub fn build() -> LockStatus {
    crate::global_lock::lock_status()
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
