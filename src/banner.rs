//! Startup banner printed to stdout when the server begins listening.

/// Print server addresses to stdout.
pub fn print(addr: &str) {
    println!("Server on http://{addr}");
    println!("  REST  http://{addr}/");
    println!("  MCP   http://{addr}/mcp");
}

#[cfg(test)]
#[path = "banner_tests.rs"]
mod banner_tests;
