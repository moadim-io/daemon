#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
        .route("/metrics", get(metrics::metrics))
        .route("/shutdown", post(shutdown::shutdown))
        .route("/restart", post(restart::restart))
        .route("/machine", get(get_current_machine).put(put_machine))
        .route("/machines", get(list_machines))
        .route(
            "/config/user-prompt",
            get(get_user_prompt).put(put_user_prompt),
        )
        .route(
            "/config/max-concurrent-runs",
            get(get_max_concurrent_runs).put(put_max_concurrent_runs),
        )
        .route("/agents", get(list_agents::list_agents))
        .route("/routines.ics", get(routines::ical_feed))
        .route(
            "/routines",
            get(list_routines::list_routines).post(create_routine::create_routine),
        )
        .route(
            "/routines/cleanup",
            post(cleanup_workbenches::cleanup_workbenches),
        )
        .route("/routines/runs", get(routines::get_all_runs))
        .route(
            "/routines/lock",
            get(get_lock_status::get_lock_status)
                .post(lock_routines::lock_routines)
                .delete(unlock_routines::unlock_routines),
        )
        .route(
            "/routines/{id}",
            get(get_routine::get_routine)
                .put(update_routine::replace)
                .patch(update_routine::update_routine)
                .delete(delete_routine::delete_routine),
        )
        .route("/routines/{id}/move", post(move_routine::move_routine))
        .route(
            "/routines/{id}/trigger",
            post(trigger_routine::trigger_routine),
        )
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
            get(list_flags::list_flags).post(create_flag::create_flag),
        )
        .route(
            "/routines/{id}/flags/{filename}",
            delete(resolve_flag::resolve_flag),
        )
        .route("/routines/{id}/logs", get(routines::get_logs))
        .route(
            "/routines/{id}/runs",
            get(list_routine_runs::list_routine_runs),
        )
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
        // Back-compat: the React client used to live under `/client` — see
        // `redirect_client_to_root`.
        .route("/client", get(redirect_client_to_root))
        .route("/client/{*rest}", get(redirect_client_to_root))
        .nest("/api/v1", api)
        .nest_service("/mcp", mcp_service)
        .merge({
            use utoipa::OpenApi as _;
            SwaggerUi::new("/docs").url("/docs/openapi.json", crate::openapi::ApiDoc::openapi())
        })
        // SPA fallback: client-routed pages (`/routines`) and refreshes on them return the app
        // HTML so React Router can resolve the path on load.
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
