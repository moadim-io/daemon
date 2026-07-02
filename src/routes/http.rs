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
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use tower_http::compression::CompressionLayer;
use utoipa_swagger_ui::SwaggerUi;

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

/// Response body for `GET /machine`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct MachineResponse {
    /// Resolved name of this machine (from `MOADIM_MACHINE`, `~/.config/moadim/machine.local.toml`, or hostname).
    pub name: String,
}

/// `GET /machine` — the current machine's resolved identity.
///
/// Returns the name this daemon uses to match `machines[]` targeting lists on routines. Useful for
/// clients (e.g. the UI) that want to default their views to local entries only.
#[utoipa::path(get, path = "/machine",
    responses((status = 200, body = MachineResponse)))]
pub async fn get_current_machine() -> Json<MachineResponse> {
    Json(MachineResponse {
        name: crate::machine::current_machine(),
    })
}

/// Request body for `PUT /machine`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetMachineRequest {
    /// New machine name. Trimmed; must be non-empty.
    pub name: String,
}

/// `PUT /machine` — rename this machine's identity.
///
/// Writes the new name to `machine.local.toml` and returns it trimmed. Returns `400` if the name
/// is empty, `500` if the write fails. The `MOADIM_MACHINE` env var takes precedence at runtime;
/// setting the name here persists it for when the env var is absent.
#[utoipa::path(put, path = "/machine",
    request_body = SetMachineRequest,
    responses(
        (status = 200, body = MachineResponse),
        (status = 400, description = "Empty name"),
        (status = 500, description = "Write failed"),
    ))]
pub async fn put_machine(
    Json(body): Json<SetMachineRequest>,
) -> Result<Json<MachineResponse>, (StatusCode, String)> {
    match crate::machine::set_machine(&body.name) {
        Ok(()) => Ok(Json(MachineResponse {
            name: body.name.trim().to_string(),
        })),
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {
            Err((StatusCode::BAD_REQUEST, err.to_string()))
        }
        Err(err) => Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    }
}

/// `GET /machines` — distinct machine names this daemon knows about.
///
/// There is no central machine registry, so the "known" set is the union of every `machines`
/// targeting list declared by a routine, plus this machine's own resolved identity
/// ([`crate::machine::current_machine`]) so the local machine is always pickable even before
/// anything targets it. Sorted and de-duplicated. Backs the UI machine picker; mirrors the
/// `moadim machine list` CLI but reads the live in-memory store instead of disk.
#[utoipa::path(get, path = "/machines",
    responses((status = 200, body = Vec<String>, description = "Known machine names, sorted")))]
pub async fn list_machines(State(state): State<AppState>) -> Json<Vec<String>> {
    use crate::utils::lock::LockRecover;
    let mut names = std::collections::BTreeSet::new();
    names.insert(crate::machine::current_machine());
    for routine in state.routines.lock_recover().values() {
        names.extend(routine.machines.iter().cloned());
    }
    Json(names.into_iter().collect())
}

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
        .route("/echo", post(echo))
        .route("/machine", get(get_current_machine).put(put_machine))
        .route("/machines", get(list_machines))
        .route("/agents", get(routines::list_agents))
        .route("/routines.ics", get(routines::ical_feed))
        .route("/routines", get(routines::list).post(routines::create))
        .route("/routines/cleanup", post(routines::cleanup))
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
        // Own fallback so unknown `/api/v1` paths return a JSON 404 instead of inheriting
        // the outer SPA fallback and answering with `index.html`/`200` (issue #270).
        .fallback(api_not_found);

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
        .with_state(app_state)
}

/// Write the generated OpenAPI spec JSON to `path`, logging a warning on failure.
///
/// Best-effort: the spec is a development convenience (committed under `apis/`), so a write
/// failure must not abort server startup. Extracted from [`run_with_listener_until`] so the
/// failure branch can be exercised against an unwritable path.
pub(crate) fn write_openapi_spec(path: &std::path::Path) {
    if let Err(err) = std::fs::write(path, crate::openapi::ApiDoc::to_json()) {
        log::warn!("could not write openapi spec: {err}");
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
    let app = build_app_with_shutdown(routines, signal.clone());
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
        .await
        .expect("axum serve failed");
    cleanup_task.abort();
    watchdog_task.abort();
    Ok(())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
