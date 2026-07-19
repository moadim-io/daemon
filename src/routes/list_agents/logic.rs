//! Shared `list_agents` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

/// Build the list of available agent registry keys a routine can launch.
pub fn build() -> Vec<String> {
    crate::routines::available_agents()
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
