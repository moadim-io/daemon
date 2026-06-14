//! HTTP server setup: builds the Axum router and starts listening.

use super::mcp::MoadimMcp;
use crate::cron_jobs::{self, new_registry, AppState, CronJob, CronStore};
use crate::middlewares;
use crate::utils::time::now_secs;
use axum::{
    extract::State,
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::OpenApi as _;
use utoipa_swagger_ui::SwaggerUi;

/// Response body returned by `GET /health`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Always `"ok"` while the server is running.
    pub status: String,
    /// Seconds elapsed since the server started.
    pub uptime_secs: u64,
    /// Always `true` while the server is running.
    pub running: bool,
}

/// Request body for `POST /echo`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct EchoRequest {
    /// Message to echo back.
    pub message: String,
}

/// Response body returned by `POST /echo`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct EchoResponse {
    /// The echoed message.
    pub message: String,
    /// Unix timestamp (seconds) when the server processed the request.
    pub timestamp: u64,
}

/// `GET /` — liveness check.
#[utoipa::path(get, path = "/",
    responses((status = 200, description = "Server is running")))]
pub async fn index() -> &'static str {
    "Moadim server is running"
}

/// `GET /health` — health status and server uptime.
#[utoipa::path(get, path = "/health",
    responses((status = 200, body = HealthResponse)))]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_secs: now_secs() - state.uptime_start,
        running: true,
    })
}

/// `POST /echo` — echo a message with a server timestamp.
#[utoipa::path(post, path = "/echo",
    request_body = EchoRequest,
    responses(
        (status = 200, body = EchoResponse),
        (status = 400, description = "Invalid JSON or missing field"),
    ))]
pub async fn echo(body: axum::body::Bytes) -> Result<Json<EchoResponse>, axum::http::StatusCode> {
    let parsed: EchoRequest =
        serde_json::from_slice(&body).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    Ok(Json(EchoResponse {
        message: parsed.message,
        timestamp: now_secs(),
    }))
}

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

    let uptime_start = now_secs();
    let app_state = AppState {
        store: store.clone(),
        handlers: new_registry(),
        uptime_start,
    };

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
        .route("/", get(index))
        .route("/health", get(health))
        .route("/echo", post(echo))
        .route("/cron-jobs", get(cron_jobs::list).post(cron_jobs::create))
        .route(
            "/cron-jobs/{id}",
            get(cron_jobs::get)
                .put(cron_jobs::replace)
                .patch(cron_jobs::update)
                .delete(cron_jobs::delete),
        )
        .route("/cron-jobs/{id}/trigger", post(cron_jobs::trigger))
        .route("/system-cron-jobs", get(list_system_cron_jobs))
        .nest_service("/mcp", mcp_service)
        .merge(
            SwaggerUi::new("/docs")
                .url("/docs/openapi.json", crate::openapi::ApiDoc::openapi()),
        )
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

    let spec_path = concat!(env!("CARGO_MANIFEST_DIR"), "/apis/openapi.json");
    if let Err(e) = std::fs::write(spec_path, crate::openapi::ApiDoc::to_json()) {
        log::warn!("could not write openapi spec: {e}");
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
