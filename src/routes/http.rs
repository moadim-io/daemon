//! HTTP server setup: builds the Axum router and starts listening.

use super::mcp::MoadimMcp;
use crate::cron_jobs::{self, new_registry, AppState, CronStore, ShutdownSignal};
use crate::middlewares;
use crate::routines::{self, RoutineStore};
use crate::utils::time::now_secs;
use axum::{
    extract::State,
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa_swagger_ui::SwaggerUi;

/// Response body for `GET /health`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Health status string (always `"ok"` when reachable).
    pub status: String,
    /// Seconds elapsed since the server started.
    pub uptime_secs: u64,
    /// Whether the server is running.
    pub running: bool,
}

/// Request body for `POST /echo`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct EchoRequest {
    /// Message to echo back.
    pub message: String,
}

/// Response body for `POST /echo`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct EchoResponse {
    /// The echoed message.
    pub message: String,
    /// Server timestamp (Unix seconds) when the echo was produced.
    pub timestamp: u64,
}

/// `GET /` — liveness check.
#[utoipa::path(get, path = "/",
    responses((status = 200, description = "Server is running", body = str)))]
pub async fn index() -> &'static str {
    "Moadim server is running"
}

/// `GET /health` — health check with uptime.
#[utoipa::path(get, path = "/health",
    responses((status = 200, body = HealthResponse)))]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_secs: now_secs() - state.uptime_start,
        running: true,
    })
}

/// Response body for `POST /shutdown`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ShutdownResponse {
    /// Acknowledgement status (always `"shutting down"`).
    pub status: String,
}

/// `POST /shutdown` — ask the server to stop gracefully.
///
/// Used by the UI "STOP" button (and the `moadim stop` command) to kill a backgrounded server that
/// has no controlling terminal. The response is sent before the graceful shutdown completes.
#[utoipa::path(post, path = "/shutdown",
    responses((status = 200, body = ShutdownResponse)))]
pub async fn shutdown(State(state): State<AppState>) -> Json<ShutdownResponse> {
    log::info!("shutdown requested via API");
    state.shutdown.notify_one();
    Json(ShutdownResponse {
        status: "shutting down".to_string(),
    })
}

/// `POST /echo` — parse a JSON body and return the message with a server timestamp.
#[utoipa::path(post, path = "/echo",
    request_body = EchoRequest,
    responses((status = 200, body = EchoResponse), (status = 400, description = "Invalid body")))]
pub async fn echo(body: axum::body::Bytes) -> Result<Json<EchoResponse>, axum::http::StatusCode> {
    let parsed: EchoRequest =
        serde_json::from_slice(&body).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    Ok(Json(EchoResponse {
        message: parsed.message,
        timestamp: now_secs(),
    }))
}

/// Build the Axum router with all routes, middleware, and state wired up.
///
/// The shutdown signal is created internally; callers that need to trigger shutdown out of band
/// (the serving loop) should use [`build_app_with_shutdown`].
#[cfg(test)]
pub(crate) fn build_app(store: CronStore, routines: RoutineStore) -> Router {
    build_app_with_shutdown(store, routines, Arc::new(tokio::sync::Notify::new()))
}

/// Build the Axum router, wiring `shutdown` into the app state so the `/shutdown` route can fire it.
pub(crate) fn build_app_with_shutdown(
    store: CronStore,
    routines: RoutineStore,
    shutdown_signal: ShutdownSignal,
) -> Router {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let app_state = AppState {
        store: store.clone(),
        handlers: new_registry(),
        routines: routines.clone(),
        uptime_start: now_secs(),
        shutdown: shutdown_signal,
    };

    let mcp_store = store.clone();
    let mcp_handlers = app_state.handlers.clone();
    let mcp_routines = routines.clone();
    let uptime_start = app_state.uptime_start;
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(MoadimMcp::new(
                mcp_store.clone(),
                mcp_handlers.clone(),
                mcp_routines.clone(),
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
        .route("/shutdown", post(shutdown))
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
        .route("/cron-jobs/{id}/logs", get(cron_jobs::get_logs))
        .route("/agents", get(routines::list_agents))
        .route("/routines", get(routines::list).post(routines::create))
        .route("/routines/cleanup", post(routines::cleanup))
        .route(
            "/routines/{id}",
            get(routines::get)
                .put(routines::replace)
                .patch(routines::update)
                .delete(routines::delete),
        )
        .route("/routines/{id}/trigger", post(routines::trigger))
        .route("/routines/{id}/logs", get(routines::get_logs))
        .nest_service("/mcp", mcp_service)
        .merge({
            use utoipa::OpenApi as _;
            SwaggerUi::new("/docs").url("/docs/openapi.json", crate::openapi::ApiDoc::openapi())
        })
        .layer(middleware::from_fn(middlewares::fs_location::fs_location))
        .layer(middleware::from_fn(middlewares::logger::logger))
        .with_state(app_state)
}

/// Serve the application on `listener`, shutting down when `shutdown` resolves.
pub async fn run_with_listener_until(
    store: CronStore,
    routines: RoutineStore,
    listener: tokio::net::TcpListener,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?.to_string();
    let spec_path = concat!(env!("CARGO_MANIFEST_DIR"), "/apis/openapi.json");
    if let Err(e) = std::fs::write(spec_path, crate::openapi::ApiDoc::to_json()) {
        log::warn!("could not write openapi spec: {e}");
    }
    let signal: ShutdownSignal = Arc::new(tokio::sync::Notify::new());
    // Periodically reap finished, expired run workbenches so triggered routines do not accumulate
    // forever (see `routines::cleanup`). The first tick fires immediately, sweeping leftovers from
    // before this process started.
    let cleanup_store = routines.clone();
    let cleanup_task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(crate::routines::CLEANUP_INTERVAL);
        loop {
            tick.tick().await;
            let store = cleanup_store.clone();
            let _ = tokio::task::spawn_blocking(move || {
                crate::routines::cleanup_expired_workbenches(&store)
            })
            .await;
        }
    });
    let app = build_app_with_shutdown(store, routines, signal.clone());
    crate::utils::startup_print::print(&addr);
    // Shut down when either the caller-supplied future resolves (e.g. a SIGINT/SIGTERM handler) or
    // the `/shutdown` route fires `signal` (the UI "STOP" button / `moadim stop`).
    let combined = async move {
        tokio::select! {
            _ = shutdown => {}
            _ = signal.notified() => {}
        }
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(combined)
        .await?;
    cleanup_task.abort();
    Ok(())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
