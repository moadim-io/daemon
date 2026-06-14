#![deny(warnings)]
//! Moadim server binary. Runs the Axum HTTP server with REST and MCP transports.

mod banner;
mod cron_jobs;
mod cron_sync;
mod error;
/// Server filesystem location helpers.
mod filesystem;
/// Axum middleware stack.
mod middlewares;
/// Filesystem path builders for the jobs directory.
mod paths;
/// HTTP and MCP route definitions.
mod routes;
/// TOML-backed job persistence.
mod storage;
/// Shared utility functions.
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = storage::load_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:5784").await?;
    routes::http::run_with_listener_until(store, listener, std::future::pending()).await
}
