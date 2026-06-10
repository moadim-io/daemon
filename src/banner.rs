pub fn print(addr: &str) {
    println!("Server on http://{addr}");
    println!("  REST  http://{addr}/");
    println!("  MCP   http://{addr}/mcp");
    println!("  UI    http://{addr}/ui");
}
