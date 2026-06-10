use axum::{
    middleware,
    routing::{get, post},
    Json, Router,
};
use std::time::SystemTime;
use utoipa::OpenApi;

use crate::cron_jobs::{self, CronJob, CreateRequest, CronStore, UpdateRequest};
use crate::middleware as mw;
use super::mcp::MoadimMcp;

#[derive(OpenApi)]
#[openapi(
    paths(
        cron_jobs::create,
        cron_jobs::list,
        cron_jobs::get,
        cron_jobs::update,
        cron_jobs::delete,
    ),
    components(schemas(CronJob, CreateRequest, UpdateRequest))
)]
pub struct ApiDoc;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub async fn run(store: CronStore) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let uptime_start = now_secs();

    let mcp_store = store.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(MoadimMcp::new(mcp_store.clone(), uptime_start)),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    let app = Router::new()
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
        .layer(middleware::from_fn(mw::logger))
        .with_state(store);

    let addr = "127.0.0.1:5784";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Server on http://{addr}");
    println!("  REST  http://{addr}/");
    println!("  MCP   http://{addr}/mcp");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn echo(body: axum::body::Bytes) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    #[derive(serde::Deserialize)]
    struct EchoRequest {
        message: String,
    }

    let parsed: EchoRequest = serde_json::from_slice(&body)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    Ok(Json(serde_json::json!({
        "message": parsed.message,
        "timestamp": now_secs(),
    })))
}
