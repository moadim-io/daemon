#![deny(warnings)]
//! Moadim server binary. Runs the Axum HTTP server with REST and MCP transports.

#[cfg(not(target_arch = "wasm32"))]
mod banner;
#[cfg(not(target_arch = "wasm32"))]
mod cron_jobs;
#[cfg(not(target_arch = "wasm32"))]
mod error;
/// Server filesystem location helpers.
#[cfg(not(target_arch = "wasm32"))]
mod fs_location;
/// Axum middleware stack.
#[cfg(not(target_arch = "wasm32"))]
mod middlewares;
/// Filesystem path builders for the jobs directory.
#[cfg(not(target_arch = "wasm32"))]
mod paths;
/// HTTP and MCP route definitions.
#[cfg(not(target_arch = "wasm32"))]
mod routes;
/// TOML-backed job persistence.
#[cfg(not(target_arch = "wasm32"))]
mod storage;
/// System crontab discovery.
#[cfg(not(target_arch = "wasm32"))]
mod system_cron;
/// Shared utility functions.
#[cfg(not(target_arch = "wasm32"))]
mod utils;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm::wasm_init();
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = storage::load_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:5784").await?;
    routes::http::run_with_listener_until(store, listener, std::future::pending()).await
}
