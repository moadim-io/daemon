//! HTTP server setup: builds the Axum router and starts listening.

use super::cleanup_workbenches;
use super::create_flag;
use super::create_routine;
use super::delete_routine;
use super::get_lock_status;
use super::get_routine;
use super::health;
use super::list_agents;
use super::list_flags;
use super::list_routine_runs;
use super::list_routines;
use super::lock_routines;
use super::mcp::MoadimMcp;
use super::metrics;
use super::move_routine;
use super::resolve_flag;
use super::restart;
use super::shutdown;
use super::trigger_routine;
use super::unlock_routines;
use super::update_routine;
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

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "build_app_with_shutdown.rs"]
mod build_app_with_shutdown;
pub(crate) use build_app_with_shutdown::*;

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

/// The embedded React `client/` SPA HTML (served at `/`), baked into the binary at compile time
/// by `src/build/client.rs`.
const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/index.html"));

/// Strong `ETag` for [`INDEX_HTML`], computed once from its content.
///
/// `DefaultHasher::new()` uses fixed keys (unlike `HashMap`'s randomized default), so this is
/// deterministic across restarts of the same binary and only changes when a new build embeds
/// different bytes. It isn't cryptographic — an `ETag` just needs to change when the content
/// does, not resist tampering.
static INDEX_ETAG: LazyLock<String> = LazyLock::new(|| etag_for(INDEX_HTML));

/// Strong `ETag` for an embedded SPA HTML body, deterministic across restarts of the same binary.
fn etag_for(html: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    html.hash(&mut hasher);
    format!("\"{:016x}\"", hasher.finish())
}

/// Serves the embedded SPA's HTML with a strong `ETag`, honoring `If-None-Match` with a bodyless
/// `304 Not Modified` so a client that already has the current build only pays for the request
/// round-trip on reload, not a re-download of the full body (issue #401). `Cache-Control:
/// no-cache` forces that revalidation on every load rather than trusting a local TTL, since the
/// content can change on any daemon upgrade.
fn serve_spa(html: &'static str, etag: &'static str, headers: &HeaderMap) -> Response {
    let not_modified = headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == etag);
    if not_modified {
        return (StatusCode::NOT_MODIFIED, [(ETAG, etag)]).into_response();
    }
    (
        [(ETAG, etag), (CACHE_CONTROL, "no-cache")],
        axum::response::Html(html),
    )
        .into_response()
}

/// `GET /` — serve the web client (single-page UI).
#[utoipa::path(get, path = "/",
    responses(
        (status = 200, description = "Web client HTML", body = str),
        (status = 304, description = "Client's cached copy is still current"),
    ))]
pub async fn index(headers: HeaderMap) -> Response {
    serve_spa(INDEX_HTML, INDEX_ETAG.as_str(), &headers)
}

/// `GET /client` (and any `/client/*` deep link) — permanent redirect to the same path at the
/// root. The React client used to be routed under `/client` (with the legacy Yew UI at `/`);
/// old bookmarks and deep links (e.g. `/client/routines?history=<id>`) still resolve.
pub async fn redirect_client_to_root(uri: axum::http::Uri) -> axum::response::Redirect {
    let path = match uri.path().strip_prefix("/client") {
        Some("") | None => "/",
        Some(rest) => rest,
    };
    match uri.query() {
        Some(query) => axum::response::Redirect::permanent(&format!("{path}?{query}")),
        None => axum::response::Redirect::permanent(path),
    }
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
include!("build_app.rs");
