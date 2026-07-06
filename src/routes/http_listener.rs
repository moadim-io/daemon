//! Server lifecycle: `OpenAPI` spec dump, graceful-shutdown grace window, and the top-level
//! `run_with_listener_until` serving loop, split out of [`super`] to keep it under the repo's
//! per-file line gate.

use super::{build_app_with_shutdown, ShutdownSignal};
use crate::routines::RoutineStore;
use std::sync::Arc;
use std::time::Duration;

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
pub(super) const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// Env override for [`SHUTDOWN_GRACE`] in milliseconds (test seam): lets tests drive the grace
/// window to a few milliseconds instead of waiting whole seconds.
pub(super) const SHUTDOWN_GRACE_MS_ENV: &str = "MOADIM_SHUTDOWN_GRACE_MS";

/// The post-shutdown drain deadline, honoring [`SHUTDOWN_GRACE_MS_ENV`] when set to a parseable
/// millisecond count; otherwise [`SHUTDOWN_GRACE`].
pub(super) fn shutdown_grace() -> Duration {
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
pub(super) async fn serve_with_grace(
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
    // Force-kill hung runs on a shorter cadence than the reap above, so a sub-minute
    // `max_runtime_secs` is enforced near its bound instead of waiting for the next sweep.
    // This tick only evaluates the kill branch; TTL reaping of the killed workbench still happens in
    // the sweep above.
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
