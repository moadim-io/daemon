//! Cron job data model, service functions, and Axum HTTP handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use croner::Cron;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::error::AppError;
use crate::paths::job_toml_path;
use crate::storage::{remove_job_dir, write_job};
use crate::utils::time::now_secs;

/// Whether a cron job is owned by this server or discovered from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CronJobSourceType {
    /// Created and managed by this server.
    Managed,
    /// Read-only entry discovered from the host OS crontab.
    System,
}

impl CronJobSourceType {
    /// Derive from the raw `source` string stored on a [`CronJob`].
    pub fn from_source(source: &str) -> Self {
        if source == "managed" {
            Self::Managed
        } else {
            Self::System
        }
    }
}

/// A persisted cron job with scheduling and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
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
    /// Unix timestamp (seconds) when the job was last manually triggered, if ever.
    pub last_triggered_at: Option<u64>,
}

/// A [`CronJob`] enriched with a flag indicating whether its handler is registered.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct CronJobResponse {
    /// The underlying cron job.
    #[serde(flatten)]
    pub job: CronJob,
    /// Whether the job is owned by this server or the host OS.
    pub source_type: CronJobSourceType,
    /// `true` if the job's handler appears in the server's handler registry.
    pub handler_registered: bool,
    /// Absolute path to the job's `job.toml` file on disk.
    pub file_path: String,
    /// Human-readable description of the schedule (e.g. `"At 09:30, Monday through Friday"`).
    /// `null` for expressions that cannot be parsed into a description (e.g. `@reboot`).
    pub schedule_description: Option<String>,
}

impl CronJobResponse {
    /// Build a response from `job`, checking `handlers` for registration status.
    pub fn from_job(job: CronJob, handlers: &HashSet<String>) -> Self {
        let source_type = CronJobSourceType::from_source(&job.source);
        let handler_registered = handlers.contains(&job.handler);
        let file_path = job_toml_path(&job.id).to_string_lossy().into_owned();
        let schedule_description = job.schedule.parse::<Cron>().ok().map(|c| c.describe());
        Self {
            job,
            source_type,
            handler_registered,
            file_path,
            schedule_description,
        }
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
#[cfg(test)]
pub fn new_store() -> CronStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Create an empty [`HandlerRegistry`].
pub fn new_registry() -> HandlerRegistry {
    Arc::new(HashSet::new())
}

/// Normalize `expr` to 5-field OS cron format for consistent storage.
///
/// Strips the seconds (field 0) and year (field 6) from any 7-field expression.
/// `@keyword` schedules and already-5-field expressions are returned unchanged.
fn normalize_schedule(expr: &str) -> String {
    let s = expr.trim();
    if s.starts_with('@') {
        return s.to_string();
    }
    let fields: Vec<&str> = s.split_ascii_whitespace().collect();
    match fields.len() {
        7 => fields[1..6].join(" "),
        _ => s.to_string(),
    }
}

/// Parse `expr` as a cron expression, returning `BadRequest` on failure.
///
/// Accepts standard 5-field (`min hour dom month dow`) and `@keyword` formats.
/// 7-field expressions are first normalized to 5-field via [`normalize_schedule`].
fn validate_cron(expr: &str) -> Result<(), AppError> {
    let normalized = normalize_schedule(expr.trim());
    normalized
        .parse::<Cron>()
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
    #[schemars(schema_with = "crate::utils::schema::metadata_schema")]
    pub metadata: serde_json::Value,
    /// Whether to create the job in an enabled state (defaults to `true`).
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

/// Serde default for boolean fields that should default to `true`.
fn bool_true() -> bool {
    true
}

/// Request body for partially updating an existing cron job.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct UpdateRequest {
    /// New cron expression, or `None` to keep the existing value.
    pub schedule: Option<String>,
    /// New handler identifier, or `None` to keep the existing value.
    pub handler: Option<String>,
    /// New metadata, or `None` to keep the existing value.
    #[schemars(schema_with = "crate::utils::schema::metadata_schema")]
    pub metadata: Option<serde_json::Value>,
    /// New enabled state, or `None` to keep the existing value.
    pub enabled: Option<bool>,
}

// --- Service layer (no HTTP types) ---

/// Return all jobs sorted by creation time (oldest first).
pub fn svc_list(store: &CronStore, handlers: &HandlerRegistry) -> Vec<CronJobResponse> {
    let lock = store.lock().unwrap();
    let mut jobs: Vec<CronJob> = lock.values().cloned().collect();
    jobs.sort_by_key(|j| j.created_at);
    drop(lock);
    jobs.into_iter()
        .map(|j| CronJobResponse::from_job(j, handlers))
        .collect()
}

/// Look up a job by `id`, returning `NotFound` if it does not exist.
pub fn svc_get(
    store: &CronStore,
    handlers: &HandlerRegistry,
    id: &str,
) -> Result<CronJobResponse, AppError> {
    let job = store
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    Ok(CronJobResponse::from_job(job, handlers))
}

/// Validate `req`, assign a UUID, persist, and return the new job.
pub fn svc_create(
    store: &CronStore,
    handlers: &HandlerRegistry,
    req: CreateRequest,
) -> Result<CronJobResponse, AppError> {
    validate_cron(&req.schedule)?;
    let now = now_secs();
    let job = CronJob {
        id: Uuid::new_v4().to_string(),
        schedule: normalize_schedule(&req.schedule),
        handler: req.handler,
        metadata: req.metadata,
        enabled: req.enabled,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_triggered_at: None,
    };
    write_job(&job).map_err(|_| AppError::Internal)?;
    store.lock().unwrap().insert(job.id.clone(), job.clone());
    if let Err(e) = crate::sync::sync_to_crontab(store) {
        log::warn!("crontab sync after create failed: {e}");
    }
    Ok(CronJobResponse::from_job(job, handlers))
}

/// Apply non-`None` fields from `req` to the job identified by `id`.
pub fn svc_update(
    store: &CronStore,
    handlers: &HandlerRegistry,
    id: &str,
    req: UpdateRequest,
) -> Result<CronJobResponse, AppError> {
    if let Some(ref sched) = req.schedule {
        validate_cron(sched)?;
    }
    let mut lock = store.lock().unwrap();
    let job = lock.get_mut(id).ok_or(AppError::NotFound)?;
    if let Some(s) = req.schedule {
        job.schedule = normalize_schedule(&s);
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
    let job = job.clone();
    drop(lock);
    write_job(&job).map_err(|_| AppError::Internal)?;
    if let Err(e) = crate::sync::sync_to_crontab(store) {
        log::warn!("crontab sync after update failed: {e}");
    }
    Ok(CronJobResponse::from_job(job, handlers))
}

/// Remove the job with `id` from the store, returning the deleted job or `NotFound`.
pub fn svc_delete(
    store: &CronStore,
    handlers: &HandlerRegistry,
    id: &str,
) -> Result<CronJobResponse, AppError> {
    let job = store.lock().unwrap().remove(id).ok_or(AppError::NotFound)?;
    remove_job_dir(id).map_err(|_| AppError::Internal)?;
    if let Err(e) = crate::sync::sync_to_crontab(store) {
        log::warn!("crontab sync after delete failed: {e}");
    }
    Ok(CronJobResponse::from_job(job, handlers))
}

/// Record a manual trigger for `id`, updating `last_triggered_at` in-store and on disk.
pub fn svc_trigger(store: &CronStore, id: &str) -> Result<CronJob, AppError> {
    let mut lock = store.lock().unwrap();
    let job = lock.get_mut(id).ok_or(AppError::NotFound)?;
    job.last_triggered_at = Some(now_secs());
    let job = job.clone();
    drop(lock);
    write_job(&job).map_err(|_| AppError::Internal)?;
    Ok(job)
}

// --- Axum HTTP handlers ---

/// `POST /cron-jobs` — create a new cron job.
#[utoipa::path(post, path = "/cron-jobs",
    request_body = CreateRequest,
    responses((status = 201, body = CronJobResponse), (status = 400, description = "Invalid cron expression")))]
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CronJobResponse>), AppError> {
    Ok((
        StatusCode::CREATED,
        Json(svc_create(&state.store, &state.handlers, body)?),
    ))
}

/// `GET /cron-jobs` — list all cron jobs sorted by creation time.
#[utoipa::path(get, path = "/cron-jobs",
    responses((status = 200, body = Vec<CronJobResponse>)))]
pub async fn list(State(state): State<AppState>) -> Json<Vec<CronJobResponse>> {
    Json(svc_list(&state.store, &state.handlers))
}

/// `GET /cron-jobs/{id}` — retrieve a single cron job by UUID.
#[utoipa::path(get, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJobResponse), (status = 404, description = "Not found")))]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CronJobResponse>, AppError> {
    Ok(Json(svc_get(&state.store, &state.handlers, &id)?))
}

/// `PATCH /cron-jobs/{id}` — partially update a cron job.
#[utoipa::path(patch, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    request_body = UpdateRequest,
    responses((status = 200, body = CronJobResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<CronJobResponse>, AppError> {
    Ok(Json(svc_update(&state.store, &state.handlers, &id, body)?))
}

/// `DELETE /cron-jobs/{id}` — delete a cron job by UUID.
#[utoipa::path(delete, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJobResponse), (status = 404, description = "Not found")))]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CronJobResponse>, AppError> {
    Ok(Json(svc_delete(&state.store, &state.handlers, &id)?))
}

/// `POST /cron-jobs/{id}/trigger` — manually trigger a cron job outside its schedule.
#[utoipa::path(post, path = "/cron-jobs/{id}/trigger",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, body = CronJob), (status = 404, description = "Not found")))]
pub async fn trigger(
    State(store): State<CronStore>,
    Path(id): Path<String>,
) -> Result<Json<CronJob>, AppError> {
    Ok(Json(svc_trigger(&store, &id)?))
}

/// Return the log file path for job `id`, or `NotFound` if no such job exists.
pub fn svc_logs_path(store: &CronStore, id: &str) -> Result<std::path::PathBuf, AppError> {
    if !store.lock().unwrap().contains_key(id) {
        return Err(AppError::NotFound);
    }
    Ok(crate::paths::job_log_path(id))
}

/// `GET /cron-jobs/{id}/logs` — return the contents of the job's log file as plain text.
#[utoipa::path(get, path = "/cron-jobs/{id}/logs",
    params(("id" = String, Path, description = "Cron job UUID")),
    responses((status = 200, description = "Log file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_logs(
    State(store): State<CronStore>,
    Path(id): Path<String>,
) -> Result<String, AppError> {
    let log_path = svc_logs_path(&store, &id)?;
    if !log_path.exists() {
        return Ok(String::new());
    }
    tokio::fs::read_to_string(&log_path)
        .await
        .map_err(|_| AppError::Internal)
}

#[cfg(test)]
#[path = "cron_jobs_tests.rs"]
mod cron_jobs_tests;
