//! Startup banner printed to stdout when the server begins listening.

/// Print server addresses to stdout.
pub fn print(addr: &str) {
    println!("Server on http://{addr}");
    println!("  UI    http://{addr}/");
    println!("  REST  http://{addr}/api/v1");
    println!("  MCP   http://{addr}/mcp");
    println!("  Docs  http://{addr}/docs");
}

#[cfg(test)]
#[path = "startup_print_tests.rs"]
mod startup_print_tests;
