//! HTTP server setup: builds the Axum router and starts listening.

use super::mcp::MoadimMcp;
use crate::cron_jobs::{self, new_registry, AppState, CronJob, CronStore};
use crate::middlewares;
use crate::utils::time::now_secs;
use axum::{
    middleware,
    routing::{get, post},
    Json, Router,
};

/// `GET /system-cron-jobs` — list read-only system cron jobs discovered from the host.
#[utoipa::path(get, path = "/system-cron-jobs",
    responses((status = 200, body = Vec<CronJob>)))]
pub async fn list_system_cron_jobs() -> Json<Vec<CronJob>> {
    Json(crate::system_cron::read_all())
}

/// Build the Axum router with all routes, middleware, and state wired up.
pub(crate) fn build_app(store: CronStore) -> Router {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let app_state = AppState {
        store: store.clone(),
        handlers: new_registry(),
    };

    let uptime_start = now_secs();
    let mcp_store = store.clone();
    let mcp_handlers = app_state.handlers.clone();
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(MoadimMcp::new(
                mcp_store.clone(),
                mcp_handlers.clone(),
                uptime_start,
            ))
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    Router::new()
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
        .route("/cron-jobs/{id}/trigger", post(cron_jobs::trigger))
        .route("/system-cron-jobs", get(list_system_cron_jobs))
        .nest_service("/mcp", mcp_service)
        .layer(middleware::from_fn(middlewares::fs_location::fs_location))
        .layer(middleware::from_fn(middlewares::logger::logger))
        .with_state(app_state)
}

/// Serve the application on `listener`, shutting down when `shutdown` resolves.
pub async fn run_with_listener_until(
    store: CronStore,
    listener: tokio::net::TcpListener,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?.to_string();
    let app = build_app(store);
    crate::banner::print(&addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
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

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
