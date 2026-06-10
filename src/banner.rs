//! Startup banner printed to stdout when the server begins listening.

/// Print server addresses to stdout.
pub fn print(addr: &str) {
    println!("Server on http://{addr}");
    println!("  REST  http://{addr}/");
    println!("  MCP   http://{addr}/mcp");
    println!("  UI    http://{addr}/ui");
}
