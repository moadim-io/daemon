#![deny(warnings)]
//! Moadim server binary. Runs the Axum HTTP server with REST and MCP transports.

#[cfg(not(target_arch = "wasm32"))]
mod banner;
#[cfg(not(target_arch = "wasm32"))]
mod cron_jobs;
#[cfg(not(target_arch = "wasm32"))]
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod fs_location;
#[cfg(not(target_arch = "wasm32"))]
mod middlewares;
#[cfg(not(target_arch = "wasm32"))]
mod routes;
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
    routes::http::run().await
}
