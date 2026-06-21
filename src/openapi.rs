//! OpenAPI 3.0 spec for the Moadim Server REST API, generated from utoipa path decorators.

#[derive(utoipa::OpenApi)]
#[openapi(
    info(title = "Moadim Server API", version = "0.1.0", description = "REST API for managing cron jobs"),
    // Relative server URL (valid per OpenAPI 3.0, resolved against the document's
    // own origin) so Swagger UI "Try it out" targets whatever address actually
    // served the page — loopback, a custom MOADIM_BIND_ADDR, or a reverse proxy —
    // instead of a hardcoded `127.0.0.1:5784` that breaks off the default bind.
    servers((url = "/api/v1", description = "This server")),
    paths(
        crate::routes::http::health,
        crate::routes::http::shutdown,
        crate::routes::http::restart,
        crate::routes::http::echo,
        crate::cron_jobs::list,
        crate::cron_jobs::create,
        crate::cron_jobs::get,
        crate::cron_jobs::replace,
        crate::cron_jobs::update,
        crate::cron_jobs::delete,
        crate::cron_jobs::trigger,
        crate::cron_jobs::get_logs,
        crate::routines::list,
        crate::routines::list_agents,
        crate::routines::create,
        crate::routines::get,
        crate::routines::replace,
        crate::routines::update,
        crate::routines::delete,
        crate::routines::trigger,
        crate::routines::cleanup,
        crate::routines::get_logs,
        crate::routines::ical_feed,
    ),
    components(schemas(
        crate::cron_jobs::CronJob,
        crate::cron_jobs::CronJobResponse,
        crate::cron_jobs::CronJobSourceType,
        crate::cron_jobs::CreateRequest,
        crate::cron_jobs::UpdateRequest,
        crate::routines::Routine,
        crate::routines::Repository,
        crate::routines::RoutineResponse,
        crate::routines::CreateRoutineRequest,
        crate::routines::UpdateRoutineRequest,
        crate::routines::CleanupResponse,
        crate::routines::RoutineSort,
        crate::routines::SortOrder,
        crate::routes::http::HealthResponse,
        crate::routes::http::ShutdownResponse,
        crate::routes::http::RestartResponse,
        crate::routes::http::EchoRequest,
        crate::routes::http::EchoResponse,
    ))
)]
/// OpenAPI document aggregating all REST paths and component schemas.
pub struct ApiDoc;

impl ApiDoc {
    /// Serialize the OpenAPI spec to a pretty-printed JSON string.
    pub fn to_json() -> String {
        use utoipa::OpenApi as _;
        serde_json::to_string_pretty(&Self::openapi()).unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "openapi_tests.rs"]
mod openapi_tests;
