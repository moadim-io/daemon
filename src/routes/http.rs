//! HTTP server setup: builds the Axum router and starts listening.

use super::health;
use super::mcp::MoadimMcp;
use super::restart;
use super::shutdown;
use crate::error::AppError;
use crate::middlewares;
use crate::routines::{self, RoutineStore};
use crate::utils::time::now_secs;
use axum::{
    http::{
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
        HeaderMap, StatusCode,
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use tower::limit::GlobalConcurrencyLimitLayer;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use utoipa_swagger_ui::SwaggerUi;

/// Maximum number of requests the server services at once, across every route.
///
/// Handlers perform blocking `crontab`/`tmux`/filesystem I/O directly on Tokio worker threads (no
/// `spawn_blocking`, #360), and the server has no per-request concurrency cap otherwise — a burst
/// of concurrent requests (or a few hung crontab calls) could exhaust the runtime's worker/blocking
/// pool and leave even `GET /health` unreachable. This bounds that blast radius: requests beyond
/// the cap simply queue for a free slot instead of piling onto more threads (#410).
const MAX_CONCURRENT_REQUESTS: usize = 64;

/// Shared signal that asks the running server to shut down gracefully.
///
/// The `/shutdown` route calls [`tokio::sync::Notify::notify_one`] on this; the serving loop awaits
/// it and begins a graceful shutdown. A stored permit means notifying before the loop registers its
/// waiter is safe (the later `notified()` returns immediately).
pub type ShutdownSignal = Arc<tokio::sync::Notify>;

/// Combined Axum application state holding the routine store.
#[derive(Clone)]
pub struct AppState {
    /// Shared routine (agent-driven job) store.
    pub routines: RoutineStore,
    /// On-disk directory the routine store is re-scanned from on every list/get request.
    /// Defaults to [`crate::paths::routines_dir`]; tests point it at a tempdir for isolation.
    pub routines_dir: std::path::PathBuf,
    /// Unix timestamp (seconds) when the server started.
    pub uptime_start: u64,
    /// Fired by the `/shutdown` route to ask the server to stop.
    pub shutdown: ShutdownSignal,
}

impl axum::extract::FromRef<AppState> for RoutineStore {
    fn from_ref(state: &AppState) -> Self {
        state.routines.clone()
    }
}

/// The embedded SPA HTML, baked into the binary at compile time.
const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/index.html"));

/// Strong `ETag` for [`INDEX_HTML`], computed once from its content.
///
/// `DefaultHasher::new()` uses fixed keys (unlike `HashMap`'s randomized default), so this is
/// deterministic across restarts of the same binary and only changes when a new build embeds
/// different bytes. It isn't cryptographic — an `ETag` just needs to change when the content
/// does, not resist tampering.
static INDEX_ETAG: LazyLock<String> = LazyLock::new(|| {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    INDEX_HTML.hash(&mut hasher);
    format!("\"{:016x}\"", hasher.finish())
});

/// `GET /` — serve the web client (single-page UI).
///
/// Sends a strong `ETag` for the ~1.1 MB embedded SPA and honors `If-None-Match` with a bodyless
/// `304 Not Modified`, so a client that already has the current build only pays for the request
/// round-trip on reload, not a re-download of the full body (issue #401). `Cache-Control:
/// no-cache` forces that revalidation on every load rather than trusting a local TTL, since the
/// content can change on any daemon upgrade.
#[utoipa::path(get, path = "/",
    responses(
        (status = 200, description = "Web client HTML", body = str),
        (status = 304, description = "Client's cached copy is still current"),
    ))]
pub async fn index(headers: HeaderMap) -> Response {
    let etag = INDEX_ETAG.as_str();
    let not_modified = headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == etag);
    if not_modified {
        return (StatusCode::NOT_MODIFIED, [(ETAG, etag)]).into_response();
    }
    (
        [(ETAG, etag), (CACHE_CONTROL, "no-cache")],
        axum::response::Html(INDEX_HTML),
    )
        .into_response()
}

/// Fallback for any unmatched path under `/api/v1` — returns a JSON `404`.
///
/// The nested API router needs its own fallback: in axum 0.8 a `nest`ed router with no
/// fallback inherits the outer one, so the SPA `.fallback(get(index))` would otherwise
/// answer an unknown `/api/v1/...` path (a typo'd or removed endpoint) with the SPA
/// `index.html` body and `200` instead of a proper `404` (issue #270). Routing it through
/// [`AppError::NotFound`] keeps the JSON error shape (`{"error":"not found"}`) consistent
/// with the handler-level 404s, while the outer SPA fallback still serves UI routes.
async fn api_not_found() -> AppError {
    AppError::NotFound
}

#[path = "http_settings_routes.rs"]
mod http_settings_routes;
#[allow(
    unused_imports,
    reason = "utoipa's OpenApi derive resolves these hidden __path_* types via crate::routes::http::__path_*, generated by #[utoipa::path] on the re-exported handlers below"
)]
pub use http_settings_routes::{
    __path_get_current_machine, __path_get_user_prompt, __path_list_machines, __path_put_machine,
    __path_put_user_prompt, get_current_machine, get_user_prompt, list_machines, put_machine,
    put_user_prompt, MachineResponse, SetMachineRequest, SetUserPromptRequest,
};

/// Build the Axum router with all routes, middleware, and state wired up.
///
/// The shutdown signal is created internally; callers that need to trigger shutdown out of band
/// (the serving loop) should use [`build_app_with_shutdown`].
#[cfg(test)]
pub(crate) fn build_app(routines: RoutineStore) -> Router {
    build_app_with_shutdown(routines, Arc::new(tokio::sync::Notify::new()))
}

/// Build the Axum router, wiring `shutdown` into the app state so the `/shutdown` route can fire it.
pub(crate) fn build_app_with_shutdown(
    routines: RoutineStore,
    shutdown_signal: ShutdownSignal,
) -> Router {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    // Clone before moving `routines` into `app_state` below — it's needed by both the REST router
    // (via `app_state`) and the MCP service closure, so exactly one clone is required. Cloning from
    // `app_state` afterward (as this used to do) produced an extra, immediately-dropped clone of the
    // `Arc` per call.
    let mcp_routines = routines.clone();

    let app_state = AppState {
        routines,
        routines_dir: crate::paths::routines_dir(),
        uptime_start: now_secs(),
        shutdown: shutdown_signal,
    };

    let mcp_routines_dir = app_state.routines_dir.clone();
    let uptime_start = app_state.uptime_start;
    let mcp_shutdown = app_state.shutdown.clone();
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(MoadimMcp::new(
                mcp_routines.clone(),
                mcp_routines_dir.clone(),
                uptime_start,
                mcp_shutdown.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    // All REST endpoints live under the `/api/v1` prefix so the root path space is free for the
    // client-routed web UI (e.g. `/routines` resolves to a UI page, not JSON).
    let api = Router::new()
        .route("/health", get(health::health))
        .route("/shutdown", post(shutdown::shutdown))
        .route("/restart", post(restart::restart))
        .route("/machine", get(get_current_machine).put(put_machine))
        .route("/machines", get(list_machines))
        .route(
            "/config/user-prompt",
            get(get_user_prompt).put(put_user_prompt),
        )
        .route("/agents", get(routines::list_agents))
        .route("/routines.ics", get(routines::ical_feed))
        .route("/routines", get(routines::list).post(routines::create))
        .route("/routines/cleanup", post(routines::cleanup))
        .route("/routines/runs", get(routines::get_all_runs))
        .route(
            "/routines/lock",
            get(routines::get_lock_status)
                .post(routines::lock)
                .delete(routines::unlock),
        )
        .route(
            "/routines/{id}",
            get(routines::get)
                .put(routines::replace)
                .patch(routines::update)
                .delete(routines::delete),
        )
        .route("/routines/{id}/trigger", post(routines::trigger))
        .route(
            "/routines/{id}/prompt-preview",
            get(routines::get_prompt_preview),
        )
        .route(
            "/routines/{id}/scheduled-trigger",
            post(routines::scheduled_trigger),
        )
        .route(
            "/routines/{id}/flags",
            get(routines::list_flags).post(routines::create_flag),
        )
        .route(
            "/routines/{id}/flags/{filename}",
            delete(routines::resolve_flag),
        )
        .route("/routines/{id}/logs", get(routines::get_logs))
        .route("/routines/{id}/runs", get(routines::get_runs))
        .route(
            "/routines/{id}/runs/{workbench}/log",
            get(routines::get_run_log),
        )
        .route(
            "/routines/{id}/runs/{workbench}/summary",
            get(routines::get_run_summary),
        )
        // Own fallback so unknown `/api/v1` paths return a JSON 404 instead of inheriting
        // the outer SPA fallback and answering with `index.html`/`200` (issue #270).
        .fallback(api_not_found)
        // Per-request deadline (issue #402): scoped to the REST API only, so the long-lived
        // `/mcp` SSE stream (nested separately below) is never subject to it.
        .layer(middleware::from_fn(middlewares::timeout::request_timeout(
            middlewares::timeout::API_REQUEST_TIMEOUT,
        )));

    Router::new()
        .route("/", get(index))
        // Back-compat: the UI used to live at `/ui`; redirect old links to the root.
        .route(
            "/ui",
            get(|| async { axum::response::Redirect::permanent("/") }),
        )
        .nest("/api/v1", api)
        .nest_service("/mcp", mcp_service)
        .merge({
            use utoipa::OpenApi as _;
            SwaggerUi::new("/docs").url("/docs/openapi.json", crate::openapi::ApiDoc::openapi())
        })
        // SPA fallback: client-routed pages (`/routines`) and refreshes on them return the app
        // HTML so the Yew router can resolve the path on load.
        .fallback(get(index))
        // Innermost of the cross-cutting layers (added first) so a rejected request's `403`
        // still gets a security-headers pass and a logged inbound/outbound pair, while still
        // running ahead of every route handler (issue #266: DNS rebinding / cross-origin abuse
        // of the unauthenticated loopback API).
        .layer(middleware::from_fn(
            middlewares::host_validation::host_validation(
                middlewares::host_validation::allowed_hosts(),
            ),
        ))
        .layer(middleware::from_fn(
            middlewares::security_headers::security_headers,
        ))
        .layer(middleware::from_fn(middlewares::logger::logger))
        // Outermost layer: negotiates `Accept-Encoding` and gzip-compresses response bodies
        // (notably the ~1.1 MB SPA `index.html` and the OpenAPI JSON under `/docs`). A no-op
        // for clients that don't advertise gzip support (issue #399).
        .layer(CompressionLayer::new())
        // Outermost of all: a panicking handler would otherwise unwind straight through Hyper,
        // resetting the connection with no response and no logged error (issue #337). Catch it
        // here and answer with a plain 500 instead.
        .layer(CatchPanicLayer::new())
        // Global cap on in-flight requests, shared across every clone of the router (see
        // MAX_CONCURRENT_REQUESTS). Placed outermost (alongside CatchPanicLayer) so it bounds
        // *all* traffic, not just the REST API under /api/v1.
        .layer(GlobalConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
        .with_state(app_state)
}

#[path = "http_listener.rs"]
mod http_listener;
pub use http_listener::run_with_listener_until;
#[cfg(test)]
use http_listener::{
    serve_with_grace, shutdown_grace, write_openapi_spec, SHUTDOWN_GRACE, SHUTDOWN_GRACE_MS_ENV,
};

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;

#[cfg(test)]
#[path = "http_routing_tests.rs"]
mod http_routing_tests;

#[cfg(test)]
#[path = "http_listener_tests.rs"]
mod http_listener_tests;
