//! HTTP server setup: builds the Axum router and starts listening.

use axum::{
    middleware,
    routing::{get, post},
    Json, Router,
};
use super::mcp::MoadimMcp;
use crate::cron_jobs;
use crate::middlewares;
use crate::utils::time::now_secs;

/// Build the router, bind to `127.0.0.1:5784`, and serve until shutdown.
pub async fn run() -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let uptime_start = now_secs();

    let mcp_service = StreamableHttpService::new(
        move || Ok(MoadimMcp::new(uptime_start)),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    let app = Router::new()
        .route(
            "/ui",
            get(|| async {
                axum::response::Html(include_str!(concat!(env!("OUT_DIR"), "/index.html")))
            }),
        )
        .route("/", get(|| async { "Moadim server is running" }))
        .route(
            "/health",
            get(move || async move {
                Json(serde_json::json!({
                    "status": "ok",
                    "uptime_secs": now_secs() - uptime_start,
                    "running": true,
                }))
            }),
        )
        .route("/echo", post(echo))
        .route("/cron-jobs", get(cron_jobs::list).post(cron_jobs::create))
        .route(
            "/cron-jobs/{id}",
            get(cron_jobs::get)
                .put(cron_jobs::update)
                .patch(cron_jobs::update)
                .delete(cron_jobs::delete),
        )
        .nest_service("/mcp", mcp_service)
        .layer(middleware::from_fn(middlewares::fs_location::fs_location))
        .layer(middleware::from_fn(middlewares::logger::logger));

    let addr = "127.0.0.1:5784";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    crate::banner::print(addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// `POST /echo` — parse a JSON body and return the message with a server timestamp.
async fn echo(body: axum::body::Bytes) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    #[derive(serde::Deserialize)]
    struct EchoRequest {
        message: String,
    }

    let parsed: EchoRequest =
        serde_json::from_slice(&body).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    Ok(Json(serde_json::json!({
        "message": parsed.message,
        "timestamp": now_secs(),
    })))
}
