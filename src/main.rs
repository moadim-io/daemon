#[cfg(not(target_arch = "wasm32"))]
mod cron_jobs;
#[cfg(not(target_arch = "wasm32"))]
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod middleware;
#[cfg(not(target_arch = "wasm32"))]
mod routes;

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
        if let Err(e) = routes::mcp::run(mcp_store).await {
            eprintln!("MCP server error: {e}");
        }
    });

    routes::http::run(store).await
}
