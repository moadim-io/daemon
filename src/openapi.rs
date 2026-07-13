//! `OpenAPI` 3.0 spec for the Moadim Server REST API, generated from utoipa path decorators.

#[derive(utoipa::OpenApi)]
#[openapi(
    // `version` is intentionally omitted so utoipa derives it from `CARGO_PKG_VERSION`,
    // keeping the spec in lockstep with the crate instead of a frozen literal (see issue #309).
    info(title = "Moadim Server API", description = "REST API for managing routines"),
    // Host-relative server URL: Swagger UI resolves "Try it out" requests against the origin the
    // docs were loaded from, so it follows a custom MOADIM_BIND_ADDR port or a reverse proxy instead
    // of a hardcoded 127.0.0.1:5784 that breaks the moment the daemon isn't bound there (issue #385).
    servers((url = "/api/v1", description = "This server")),
    paths(
        crate::routes::health::health,
        crate::routes::http::shutdown,
        crate::routes::http::restart,
        crate::routes::http::get_current_machine,
        crate::routes::http::put_machine,
        crate::routes::http::list_machines,
        crate::routes::http::get_user_prompt,
        crate::routes::http::put_user_prompt,
        crate::routines::list,
        crate::routines::list_agents,
        crate::routines::create,
        crate::routines::get,
        crate::routines::get_prompt_preview,
        crate::routines::replace,
        crate::routines::update,
        crate::routines::delete,
        crate::routines::trigger,
        crate::routines::scheduled_trigger,
        crate::routines::cleanup,
        crate::routines::get_lock_status,
        crate::routines::lock,
        crate::routines::unlock,
        crate::routines::get_logs,
        crate::routines::get_runs,
        crate::routines::get_run_log,
        crate::routines::get_run_summary,
        crate::routines::get_all_runs,
        crate::routines::ical_feed,
        crate::routines::create_flag,
        crate::routines::list_flags,
        crate::routines::resolve_flag,
    ),
    components(schemas(
        crate::routines::Routine,
        crate::routines::Repository,
        crate::routines::RoutineResponse,
        crate::routines::CreateRoutineRequest,
        crate::routines::UpdateRoutineRequest,
        crate::routines::CleanupResponse,
        crate::routines::RunSummary,
        crate::routines::RunStatus,
        crate::routines::FleetRunSummary,
        crate::routines::RoutineSort,
        crate::routines::SortOrder,
        crate::routines::Flag,
        crate::routines::FlagScope,
        crate::routines::CreateFlagRequest,
        crate::routes::health::HealthResponse,
        crate::routes::health::DependencyHealth,
        crate::routes::http::ShutdownResponse,
        crate::routes::http::RestartResponse,
        crate::routes::http::MachineResponse,
        crate::routes::http::SetMachineRequest,
        crate::routes::http::SetUserPromptRequest,
        crate::global_lock::LockStatus,
        crate::routines::LockRequest,
    ))
)]
/// `OpenAPI` document aggregating all REST paths and component schemas.
pub struct ApiDoc;

impl ApiDoc {
    /// Serialize the `OpenAPI` spec to a pretty-printed JSON string.
    pub fn to_json() -> String {
        use utoipa::OpenApi as _;
        serde_json::to_string_pretty(&Self::openapi()).unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "openapi_tests.rs"]
mod openapi_tests;
