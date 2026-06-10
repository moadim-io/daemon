#[cfg(not(target_arch = "wasm32"))]
mod config;

#[cfg(not(target_arch = "wasm32"))]
mod cron_jobs;

#[cfg(not(target_arch = "wasm32"))]
mod error;

#[cfg(not(target_arch = "wasm32"))]
mod handlers;

#[cfg(not(target_arch = "wasm32"))]
mod middleware;

#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
mod static_handler;

mod wasm;

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm::wasm_init();
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    server::run().await
}
