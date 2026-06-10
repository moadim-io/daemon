//! Cron job data model, service functions, and Axum HTTP handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use cron::Schedule;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

use crate::error::AppError;

/// A persisted cron job with scheduling and metadata.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct CronJob {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Cron expression defining when the job runs.
    pub schedule: String,
    /// Identifier for the handler that processes the job.
    pub handler: String,
    /// Arbitrary JSON metadata attached to the job.
    pub metadata: serde_json::Value,
    /// Whether the job is active.
    pub enabled: bool,
    /// `"managed"` for jobs owned by this server; `"system:*"` for read-only system cron entries.
    pub source: String,
    /// Unix timestamp (seconds) when the job was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) when the job was last updated.
    pub updated_at: u64,
}

/// A [`CronJob`] enriched with a flag indicating whether its handler is registered.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct CronJobResponse {
    /// The underlying cron job.
    #[serde(flatten)]
    pub job: CronJob,
    /// `true` if the job's handler appears in the server's handler registry.
    pub handler_registered: bool,
}

impl CronJobResponse {
    /// Build a response from `job`, checking `handlers` for registration status.
    pub fn from_job(job: CronJob, handlers: &HashSet<String>) -> Self {
        let handler_registered = handlers.contains(&job.handler);
        Self { job, handler_registered }
    }
}

/// Thread-safe shared store of cron jobs keyed by ID.
pub type CronStore = Arc<Mutex<HashMap<String, CronJob>>>;
/// Thread-safe set of registered handler identifiers.
pub type HandlerRegistry = Arc<HashSet<String>>;

/// Combined Axum application state holding the job store and handler registry.
#[derive(Clone)]
pub struct AppState {
    /// Shared cron job store.
    pub store: CronStore,
    /// Registered handler identifiers.
    pub handlers: HandlerRegistry,
}

impl axum::extract::FromRef<AppState> for CronStore {
    fn from_ref(state: &AppState) -> Self {
        state.store.clone()
    }
}

/// Create an empty [`CronStore`].
pub fn new_store() -> CronStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Create an empty [`HandlerRegistry`].
pub fn new_registry() -> HandlerRegistry {
    Arc::new(HashSet::new())
}

/// Return current Unix time in whole seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Parse `expr` as a cron expression, returning `BadRequest` on failure.
fn validate_cron(expr: &str) -> Result<(), AppError> {
    Schedule::from_str(expr)
        .map_err(|e| AppError::BadRequest(format!("invalid cron expression: {}", e)))?;
    Ok(())
}

/// Request body for creating a new cron job.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct CreateRequest {
    /// Cron expression for the new job.
    pub schedule: String,
    /// Handler identifier to invoke when the schedule fires.
    pub handler: String,
    /// Optional metadata (defaults to null).
    #[serde(default)]
    #[schemars(schema_with = "metadata_schema")]
    pub metadata: serde_json::Value,
    /// Whether to create the job in an enabled state (defaults to `true`).
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

/// Serde default for boolean fields that should default to `true`.
fn bool_true() -> bool {
    true
}

/// Schema override that marks `metadata` as a free-form JSON object.
fn metadata_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "object", "additionalProperties": true})
}

/// Request body for partially updating an existing cron job.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct UpdateRequest {
    /// New cron expression, or `None` to keep the existing value.
    pub schedule: Option<String>,
    /// New handler identifier, or `None` to keep the existing value.
    pub handler: Option<String>,
    /// New metadata, or `None` to keep the existing value.
    #[schemars(schema_with = "metadata_schema")]
    pub metadata: Option<serde_json::Value>,
    /// New enabled state, or `None` to keep the existing value.
    pub enabled: Option<bool>,
}

// --- Service layer (no HTTP types) ---

/// Return all jobs sorted by creation time (oldest first).
pub fn svc_list(store: &CronStore) -> Vec<CronJob> {
    let lock = store.lock().unwrap();
    let mut jobs: Vec<CronJob> = lock.values().cloned().collect();
    jobs.sort_by_key(|j| j.created_at);
    jobs
}

/// Look up a job by `id`, returning `NotFound` if it does not exist.
pub fn svc_get(store: &CronStore, id: &str) -> Result<CronJob, AppError> {
    store.lock().unwrap().get(id).cloned().ok_or(AppError::NotFound)
}

/// Validate `req`, assign a UUID, persist, and return the new job.
pub fn svc_create(store: &CronStore, req: CreateRequest) -> Result<CronJob, AppError> {
    validate_cron(&req.schedule)?;
    let now = now_secs();
    let job = CronJob {
        id: Uuid::new_v4().to_string(),
        schedule: req.schedule,
        handler: req.handler,
        metadata: req.metadata,
        enabled: req.enabled,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
    };
    store.lock().unwrap().insert(job.id.clone(), job.clone());
    Ok(job)
}

/// Apply non-`None` fields from `req` to the job identified by `id`.
pub fn svc_update(store: &CronStore, id: &str, req: UpdateRequest) -> Result<CronJob, AppError> {
    if let Some(ref sched) = req.schedule {
        validate_cron(sched)?;
    }
    let mut lock = store.lock().unwrap();
    let job = lock.get_mut(id).ok_or(AppError::NotFound)?;
    if let Some(s) = req.schedule {
        job.schedule = s;
    }
    if let Some(h) = req.handler {
        job.handler = h;
    }
    if let Some(m) = req.metadata {
        job.metadata = m;
    }
    if let Some(e) = req.enabled {
        job.enabled = e;
    }
    job.updated_at = now_secs();
    Ok(job.clone())
}

/// Remove the job with `id` from the store, returning `NotFound` if absent.
pub fn svc_delete(store: &CronStore, id: &str) -> Result<(), AppError> {
    store
        .lock()
        .unwrap()
        .remove(id)
        .ok_or(AppError::NotFound)
        .map(|_| ())
}

// --- Axum HTTP handlers ---

/// `POST /cron-jobs` — create a new cron job.
#[utoipa::path(post, path = "/cron-jobs",
    request_body = CreateRequest,
    responses((status = 201, body = CronJob), (status = 400, description = "Invalid cron expression")))]
pub async fn create(
    State(store): State<CronStore>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CronJob>), AppError> {
    let job = svc_create(&store, body)?;
    Ok((StatusCode::CREATED, Json(job)))
}

/// `GET /cron-jobs` — list all cron jobs sorted by creation time.
#[utoipa::path(get, path = "/cron-jobs",
    responses((status = 200, body = Vec<CronJobResponse>)))]
pub async fn list(State(state): State<AppState>) -> Json<Vec<CronJobResponse>> {
    let jobs = svc_list(&state.store);
    Json(jobs.into_iter().map(|j| CronJobResponse::from_job(j, &state.handlers)).collect())
}

/// `GET /cron-jobs/{id}` — retrieve a single cron job by UUID.
#[utoipa::path(get, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJobResponse), (status = 404, description = "Not found")))]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CronJobResponse>, AppError> {
    let job = svc_get(&state.store, &id)?;
    Ok(Json(CronJobResponse::from_job(job, &state.handlers)))
}

/// `PATCH /cron-jobs/{id}` — partially update a cron job.
#[utoipa::path(patch, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    request_body = UpdateRequest,
    responses((status = 200, body = CronJob), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update(
    State(store): State<CronStore>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<CronJob>, AppError> {
    Ok(Json(svc_update(&store, &id, body)?))
}

/// `DELETE /cron-jobs/{id}` — delete a cron job by UUID.
#[utoipa::path(delete, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 204, description = "Deleted"), (status = 404, description = "Not found")))]
pub async fn delete(
    State(store): State<CronStore>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    svc_delete(&store, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_cron_accepts_valid() {
        assert!(validate_cron("0 30 9 * * 1-5 *").is_ok());
        assert!(validate_cron("@daily").is_ok());
    }

    #[test]
    fn validate_cron_rejects_invalid() {
        assert!(validate_cron("not a cron").is_err());
        assert!(validate_cron("99 99 99 99 99").is_err());
    }

    #[test]
    fn cron_job_serializes() {
        let job = CronJob {
            id: "abc".to_string(),
            schedule: "0 * * * * * *".to_string(),
            handler: "my-handler".to_string(),
            metadata: serde_json::json!({}),
            enabled: true,
            source: "managed".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("\"id\":\"abc\""));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn create_request_defaults_enabled_true() {
        let json = r#"{"schedule":"@daily","handler":"h"}"#;
        let req: CreateRequest = serde_json::from_str(json).unwrap();
        assert!(req.enabled);
    }

    #[test]
    fn svc_create_and_get() {
        let store = new_store();
        let req = CreateRequest {
            schedule: "@hourly".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            enabled: true,
        };
        let job = svc_create(&store, req).unwrap();
        let fetched = svc_get(&store, &job.id).unwrap();
        assert_eq!(fetched.id, job.id);
    }

    #[test]
    fn svc_delete_removes_job() {
        let store = new_store();
        let req = CreateRequest {
            schedule: "@daily".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            enabled: true,
        };
        let job = svc_create(&store, req).unwrap();
        svc_delete(&store, &job.id).unwrap();
        assert!(svc_get(&store, &job.id).is_err());
    }

    #[test]
    fn svc_update_enabled_override() {
        let store = new_store();
        let req = CreateRequest {
            schedule: "@daily".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            enabled: true,
        };
        let job = svc_create(&store, req).unwrap();
        let updated = svc_update(
            &store,
            &job.id,
            UpdateRequest {
                schedule: None,
                handler: None,
                metadata: None,
                enabled: Some(false),
            },
        )
        .unwrap();
        assert!(!updated.enabled);
    }
}
