//! OpenAPI 3.0 spec for the Moadim Server REST API, generated from utoipa path decorators.

#[derive(utoipa::OpenApi)]
#[openapi(
    info(title = "Moadim Server API", version = "0.1.0", description = "REST API for managing cron jobs"),
    servers((url = "http://127.0.0.1:5784", description = "Local development")),
    paths(
        crate::routes::http::index,
        crate::routes::http::health,
        crate::routes::http::shutdown,
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
        crate::routines::create,
        crate::routines::get,
        crate::routines::replace,
        crate::routines::update,
        crate::routines::delete,
        crate::routines::trigger,
        crate::routines::get_logs,
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
        crate::routes::http::HealthResponse,
        crate::routes::http::ShutdownResponse,
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
