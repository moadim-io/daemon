use async_graphql::{Context, EmptySubscription, InputObject, Object, Schema, SimpleObject};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{response::Html, Extension};

use crate::cron_jobs::{AppState, CronJob, CronJobResponse, CreateRequest, UpdateRequest};

// Arbitrary JSON scalar backed by serde_json::Value.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JSON(pub serde_json::Value);
async_graphql::scalar!(JSON);

// --- Output types ---

#[derive(SimpleObject)]
struct GqlCronJob {
    id: String,
    schedule: String,
    handler: String,
    metadata: JSON,
    enabled: bool,
    source: String,
    created_at: u64,
    updated_at: u64,
}

impl From<CronJob> for GqlCronJob {
    fn from(j: CronJob) -> Self {
        Self {
            id: j.id,
            schedule: j.schedule,
            handler: j.handler,
            metadata: JSON(j.metadata),
            enabled: j.enabled,
            source: j.source,
            created_at: j.created_at,
            updated_at: j.updated_at,
        }
    }
}

#[derive(SimpleObject)]
struct GqlCronJobResponse {
    id: String,
    schedule: String,
    handler: String,
    metadata: JSON,
    enabled: bool,
    source: String,
    created_at: u64,
    updated_at: u64,
    handler_registered: bool,
}

impl From<CronJobResponse> for GqlCronJobResponse {
    fn from(r: CronJobResponse) -> Self {
        Self {
            id: r.job.id,
            schedule: r.job.schedule,
            handler: r.job.handler,
            metadata: JSON(r.job.metadata),
            enabled: r.job.enabled,
            source: r.job.source,
            created_at: r.job.created_at,
            updated_at: r.job.updated_at,
            handler_registered: r.handler_registered,
        }
    }
}

#[derive(SimpleObject)]
struct GqlHealth {
    status: String,
    uptime_secs: u64,
    running: bool,
}

// --- Input types ---

#[derive(InputObject)]
struct CreateCronJobInput {
    schedule: String,
    handler: String,
    metadata: Option<JSON>,
    #[graphql(default = true)]
    enabled: bool,
}

#[derive(InputObject)]
struct UpdateCronJobInput {
    schedule: Option<String>,
    handler: Option<String>,
    metadata: Option<JSON>,
    enabled: Option<bool>,
}

// --- Query ---

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn cron_jobs(&self, ctx: &Context<'_>) -> Vec<GqlCronJobResponse> {
        let state = ctx.data_unchecked::<AppState>();
        crate::cron_jobs::svc_list(&state.store)
            .into_iter()
            .map(|j| CronJobResponse::from_job(j, &state.handlers))
            .map(GqlCronJobResponse::from)
            .collect()
    }

    async fn cron_job(&self, ctx: &Context<'_>, id: String) -> Option<GqlCronJobResponse> {
        let state = ctx.data_unchecked::<AppState>();
        crate::cron_jobs::svc_get(&state.store, &id)
            .ok()
            .map(|j| CronJobResponse::from_job(j, &state.handlers))
            .map(GqlCronJobResponse::from)
    }

    async fn system_cron_jobs(&self) -> Vec<GqlCronJob> {
        crate::system_cron::read_all()
            .into_iter()
            .map(GqlCronJob::from)
            .collect()
    }

    async fn health(&self, ctx: &Context<'_>) -> GqlHealth {
        let &uptime_start = ctx.data_unchecked::<u64>();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        GqlHealth { status: "ok".to_string(), uptime_secs: now - uptime_start, running: true }
    }
}

// --- Mutation ---

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn create_cron_job(
        &self,
        ctx: &Context<'_>,
        input: CreateCronJobInput,
    ) -> async_graphql::Result<GqlCronJob> {
        let state = ctx.data_unchecked::<AppState>();
        let req = CreateRequest {
            schedule: input.schedule,
            handler: input.handler,
            metadata: input.metadata.map(|j| j.0).unwrap_or_default(),
            enabled: input.enabled,
        };
        crate::cron_jobs::svc_create(&state.store, req)
            .map(GqlCronJob::from)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    async fn update_cron_job(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateCronJobInput,
    ) -> async_graphql::Result<GqlCronJob> {
        let state = ctx.data_unchecked::<AppState>();
        let req = UpdateRequest {
            schedule: input.schedule,
            handler: input.handler,
            metadata: input.metadata.map(|j| j.0),
            enabled: input.enabled,
        };
        crate::cron_jobs::svc_update(&state.store, &id, req)
            .map(GqlCronJob::from)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    async fn delete_cron_job(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> async_graphql::Result<String> {
        let state = ctx.data_unchecked::<AppState>();
        crate::cron_jobs::svc_delete(&state.store, &id)
            .map(|()| id)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }
}

// --- Schema ---

pub type MoadimSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema(state: AppState, uptime_start: u64) -> MoadimSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(state)
        .data(uptime_start)
        .finish()
}

// --- Axum handlers ---

pub async fn handler(
    Extension(schema): Extension<MoadimSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub async fn playground() -> Html<String> {
    Html(async_graphql::http::GraphiQLSource::build().endpoint("/graphql").finish())
}
