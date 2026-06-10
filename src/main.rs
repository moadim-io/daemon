#[cfg(not(target_arch = "wasm32"))]
mod cron_jobs;
#[cfg(not(target_arch = "wasm32"))]
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod mcp;
#[cfg(not(target_arch = "wasm32"))]
mod middleware;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm::wasm_init();
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let store = cron_jobs::new_store();

    let mcp_store = store.clone();
    tokio::spawn(async move {
        if let Err(e) = mcp::run(mcp_store).await {
            eprintln!("MCP server error: {e}");
        }
    });

    server::run(store).await
}
