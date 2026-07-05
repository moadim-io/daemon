//! HTTP server setup: builds the Axum router and starts listening.

use super::mcp::MoadimMcp;
use crate::error::AppError;
use crate::middlewares;
use crate::routines::{self, RoutineStore};
use crate::utils::time::now_secs;
use axum::{
    extract::State,
    http::{
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
        HeaderMap, StatusCode,
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
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

/// External-binary dependencies the daemon relies on at runtime, and whether each is resolvable on
/// the daemon's `PATH`. Surfaced in [`HealthResponse`] so the UI/CLI can flag a missing dependency
/// instead of having routine runs silently no-op.
#[derive(Serialize, utoipa::ToSchema)]
pub struct DependencyHealth {
    /// Whether `tmux` (used to launch every routine agent) resolves on the daemon's `PATH`.
    pub tmux: bool,
    /// Whether `python3` resolves on the daemon's `PATH`. The built-in `claude` agent's `setup`
    /// step runs a `python3` snippet to pre-seed workspace-trust state; when it is missing that
    /// step fails silently and the routine still shows a healthy status (issue #404).
    pub python3: bool,
}

/// Response body for `GET /health`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Health status string (always `"ok"` when reachable).
    pub status: String,
    /// Seconds elapsed since the server started.
    pub uptime_secs: u64,
    /// Whether the server is running.
    pub running: bool,
    /// Resolved name of this machine (from `MOADIM_MACHINE`, `~/.config/moadim/machine.local.toml`, or hostname).
    pub machine: String,
    /// Presence of required external binaries on the daemon's `PATH`.
    pub dependencies: DependencyHealth,
    /// Daemon version (from `CARGO_PKG_VERSION`).
    pub version: String,
    /// Short git commit SHA the daemon was built from, or `"unknown"` outside a git checkout.
    pub git_sha: String,
    /// Committer date (`YYYY-MM-DD`) of the build commit, or `"unknown"` outside a git checkout.
    pub build_date: String,
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

/// `GET /health` — health check with uptime.
#[utoipa::path(get, path = "/health",
    responses((status = 200, body = HealthResponse)))]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        // saturating_sub so a backward wall-clock adjustment can't underflow
        // (panic in debug, wrap to a huge value in release) — clamp to 0 instead.
        uptime_secs: now_secs().saturating_sub(state.uptime_start),
        running: true,
        machine: crate::machine::current_machine(),
        dependencies: DependencyHealth {
            tmux: routines::tmux_available(),
            python3: routines::agent_command_available("python3"),
        },
        version: crate::build_info::VERSION.to_string(),
        git_sha: crate::build_info::GIT_SHA.to_string(),
        build_date: crate::build_info::BUILD_DATE.to_string(),
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

/// Response body for `POST /restart`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct RestartResponse {
    /// Acknowledgement status (always `"restarting"`).
    pub status: String,
    /// PID of the detached helper process performing the stop-old-then-start-new restart.
    pub helper_pid: u32,
}

/// `POST /restart` — stop this server and start a fresh instance.
///
/// The running server cannot rebind its own port, so it spawns a detached `moadim restart` helper
/// that stops it and starts a new process, mirroring the `moadim restart` CLI command and the
/// `restart` MCP tool. Responds with the helper's PID before the restart completes.
#[utoipa::path(post, path = "/restart",
    responses((status = 200, body = RestartResponse), (status = 500, description = "could not spawn the restart helper")))]
pub async fn restart() -> Result<Json<RestartResponse>, AppError> {
    let helper_pid = crate::cli::spawn_restart().map_err(|_| AppError::Internal)?;
    Ok(Json(RestartResponse {
        status: "restarting".to_string(),
        helper_pid,
    }))
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
        uptime_start: now_secs(),
        shutdown: shutdown_signal,
    };

    let uptime_start = app_state.uptime_start;
    let mcp_shutdown = app_state.shutdown.clone();
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(MoadimMcp::new(
                mcp_routines.clone(),
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
        .route("/health", get(health))
        .route("/shutdown", post(shutdown))
        .route("/restart", post(restart))
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

/// Write the generated `OpenAPI` spec JSON to `path`, logging a warning on failure.
///
/// Best-effort: the spec is a development convenience (committed under `apis/`), so a write
/// failure must not abort server startup. Extracted from [`run_with_listener_until`] so the
/// failure branch can be exercised against an unwritable path.
///
/// `path` is `CARGO_MANIFEST_DIR/apis/openapi.json`, baked in at compile time. For an installed
/// binary (`cargo install`), that directory is wherever the crate happened to build, which
/// generally doesn't exist on the end user's machine — skip silently rather than warning on
/// every startup for a path nobody expects to be writable (#319).
pub(crate) fn write_openapi_spec(path: &std::path::Path) {
    if !path.parent().is_some_and(std::path::Path::is_dir) {
        return;
    }
    if let Err(err) = std::fs::write(path, crate::openapi::ApiDoc::to_json()) {
        log::warn!("could not write openapi spec: {err}");
    }
}

/// Default window granted to in-flight connections to drain after a shutdown is requested, before
/// the server is forced to return. Bounds `moadim stop`: axum's `with_graceful_shutdown` waits for
/// every open connection to close, so a never-ending stream (e.g. an `/mcp` SSE subscription) would
/// otherwise pin the process open forever (#342).
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// Env override for [`SHUTDOWN_GRACE`] in milliseconds (test seam): lets tests drive the grace
/// window to a few milliseconds instead of waiting whole seconds.
const SHUTDOWN_GRACE_MS_ENV: &str = "MOADIM_SHUTDOWN_GRACE_MS";

/// The post-shutdown drain deadline, honoring [`SHUTDOWN_GRACE_MS_ENV`] when set to a parseable
/// millisecond count; otherwise [`SHUTDOWN_GRACE`].
fn shutdown_grace() -> Duration {
    std::env::var(SHUTDOWN_GRACE_MS_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .map_or(SHUTDOWN_GRACE, Duration::from_millis)
}

/// Await `serve`, but once `shutdown_started` fires, allow open connections at most `grace` to
/// drain before forcing the server to return.
///
/// Axum's graceful shutdown blocks until every in-flight connection closes; a long-lived stream
/// (an `/mcp` SSE subscription, a slow client) can keep that future pending indefinitely, hanging
/// `moadim stop`/`POST /shutdown` forever (#342). This wrapper caps that wait: it returns `serve`'s
/// own result if the server drains on its own, or `Ok(())` after logging a warning once the grace
/// window elapses.
async fn serve_with_grace(
    serve: impl std::future::IntoFuture<Output = std::io::Result<()>>,
    shutdown_started: impl std::future::Future<Output = ()>,
    grace: Duration,
) -> std::io::Result<()> {
    // `axum::serve(..).with_graceful_shutdown(..)` is an `IntoFuture`, not a `Future`; normalize it
    // (and any plain future the tests pass) before pinning.
    let serve = serve.into_future();
    tokio::pin!(serve);
    // Phase 1: serve normally until it returns on its own or a shutdown is requested.
    tokio::select! {
        res = &mut serve => return res,
        _ = shutdown_started => {}
    }
    // Phase 2: shutdown requested — give open connections a bounded window to drain, then force exit.
    tokio::select! {
        res = &mut serve => res,
        _ = tokio::time::sleep(grace) => {
            log::warn!(
                "graceful shutdown exceeded {grace:?}; forcing exit with connections still open"
            );
            Ok(())
        }
    }
}

/// Serve the application on `listener`, shutting down when `shutdown` resolves.
pub async fn run_with_listener_until(
    routines: RoutineStore,
    listener: tokio::net::TcpListener,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let addr = listener
        .local_addr()
        .expect("TCP listener always has a local address")
        .to_string();
    write_openapi_spec(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/apis/openapi.json"
    )));
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
    // Force-kill hung runs on a much shorter cadence than the hourly reap above, so a sub-hour
    // `max_runtime_secs` is enforced near its bound instead of waiting up to ~1h for the next sweep.
    // This tick only evaluates the kill branch; TTL reaping of the killed workbench still happens in
    // the hourly sweep.
    let watchdog_store = routines.clone();
    let watchdog_task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(crate::routines::WATCHDOG_INTERVAL);
        loop {
            tick.tick().await;
            let store = watchdog_store.clone();
            let _ =
                tokio::task::spawn_blocking(move || crate::routines::kill_hung_sessions(&store))
                    .await;
        }
    });
    // Periodically warn when the binary on disk has moved on from the one this process is running
    // (#167): an in-place upgrade with no daemon restart otherwise regenerates every routine's
    // agent instructions — disclosure included — from stale, silently outdated logic.
    let version_task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(crate::build_info::VERSION_DRIFT_CHECK_INTERVAL);
        loop {
            tick.tick().await;
            let _ = tokio::task::spawn_blocking(|| {
                if let Ok(exe) = std::env::current_exe() {
                    let running = format!("moadim {}", crate::build_info::long_version());
                    crate::build_info::warn_on_drift(&exe, &running);
                }
            })
            .await;
        }
    });
    let app = build_app_with_shutdown(routines, signal.clone());
    crate::utils::startup_print::print(&addr);
    // Fires the instant a shutdown is requested, so the grace watchdog below can start its clock
    // independently of how long the in-flight connections take to drain.
    let shutdown_started: ShutdownSignal = Arc::new(tokio::sync::Notify::new());
    let started = shutdown_started.clone();
    // Shut down when either the caller-supplied future resolves (e.g. a SIGINT/SIGTERM handler) or
    // the `/shutdown` route fires `signal` (the UI "STOP" button / `moadim stop`).
    let combined = async move {
        tokio::select! {
            _ = shutdown => {}
            _ = signal.notified() => {}
        }
        started.notify_one();
    };
    let serve = axum::serve(listener, app).with_graceful_shutdown(combined);
    // Cap the post-shutdown wait so a connection that never closes (e.g. an open `/mcp` SSE stream)
    // can't pin the process open forever and hang `moadim stop` (#342).
    serve_with_grace(serve, shutdown_started.notified(), shutdown_grace()).await?;
    cleanup_task.abort();
    watchdog_task.abort();
    version_task.abort();
    Ok(())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;

#[cfg(test)]
#[path = "http_routing_tests.rs"]
mod http_routing_tests;

#[cfg(test)]
#[path = "http_listener_tests.rs"]
mod http_listener_tests;
