//! OpenAPI 3.0 spec for the Moadim Server REST API, generated from utoipa path decorators.

#[derive(utoipa::OpenApi)]
#[openapi(
    info(title = "Moadim Server API", version = "0.1.0", description = "REST API for managing cron jobs"),
    servers((url = "http://127.0.0.1:5784", description = "Local development")),
    paths(
        crate::routes::http::index,
        crate::routes::http::health,
        crate::routes::http::echo,
        crate::cron_jobs::list,
        crate::cron_jobs::create,
        crate::cron_jobs::get,
        crate::cron_jobs::replace,
        crate::cron_jobs::update,
        crate::cron_jobs::delete,
        crate::cron_jobs::trigger,
    ),
    components(schemas(
        crate::cron_jobs::CronJob,
        crate::cron_jobs::CronJobResponse,
        crate::cron_jobs::CronJobSourceType,
        crate::cron_jobs::CreateRequest,
        crate::cron_jobs::UpdateRequest,
        crate::routes::http::HealthResponse,
        crate::routes::http::EchoRequest,
        crate::routes::http::EchoResponse,
    ))
)]
pub struct ApiDoc;

impl ApiDoc {
    pub fn to_json() -> String {
        use utoipa::OpenApi as _;
        serde_json::to_string_pretty(&Self::openapi()).unwrap_or_default()
    }
}
