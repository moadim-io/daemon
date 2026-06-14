#![deny(warnings)]
//! Moadim server binary. Runs the Axum HTTP server with REST and MCP transports.

mod cron_jobs;
mod error;
/// Server filesystem location helpers.
mod filesystem;
/// Axum middleware stack.
mod middlewares;
mod openapi;
/// Filesystem path builders for the jobs directory.
mod paths;
/// HTTP and MCP route definitions.
mod routes;
/// TOML-backed routine persistence.
mod routine_storage;
/// Routine (agent-driven job) data model, service layer, and handlers.
mod routines;
/// TOML-backed job persistence.
mod storage;
/// Bidirectional sync between managed jobs and the OS crontab.
mod sync;
/// Shared utility functions.
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = storage::load_store();
    let routines = routine_storage::load_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:5784").await?;
    routes::http::run_with_listener_until(store, routines, listener, std::future::pending()).await
}
