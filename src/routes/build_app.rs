pub use http_settings_routes::{
    __path_get_current_machine, __path_get_max_concurrent_runs, __path_get_user_prompt,
    __path_list_machines, __path_put_machine, __path_put_max_concurrent_runs,
    __path_put_user_prompt, get_current_machine, get_max_concurrent_runs, get_user_prompt,
    list_machines, put_machine, put_max_concurrent_runs, put_user_prompt, MachineResponse,
    MaxConcurrentRunsResponse, SetMachineRequest, SetMaxConcurrentRunsRequest,
    SetUserPromptRequest,
};

/// Build the Axum router with all routes, middleware, and state wired up.
///
/// The shutdown signal is created internally; callers that need to trigger shutdown out of band
/// (the serving loop) should use [`build_app_with_shutdown`].
#[cfg(test)]
pub(crate) fn build_app(routines: RoutineStore) -> Router {
    build_app_with_shutdown(routines, Arc::new(tokio::sync::Notify::new()))
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
#[path = "http_auth_tests.rs"]
mod http_auth_tests;

#[cfg(test)]
#[path = "http_routing_tests.rs"]
mod http_routing_tests;

#[cfg(test)]
#[path = "http_listener_tests.rs"]
mod http_listener_tests;
