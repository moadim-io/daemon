//! Cron job domain model, service layer, and request/response types.

use cron::Schedule;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
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
}

impl CronJobResponse {
    /// Build a response from `job`, checking `handlers` for registration status.
    pub fn from_job(job: CronJob, handlers: &HashSet<String>) -> Self {
        let source_type = CronJobSourceType::from_source(&job.source);
        let handler_registered = handlers.contains(&job.handler);
        let file_path = job_toml_path(&job.id).to_string_lossy().into_owned();
        Self {
            job,
            source_type,
            handler_registered,
            file_path,
        }
    }
}

/// Thread-safe shared store of cron jobs keyed by ID.
pub type CronStore = Arc<Mutex<HashMap<String, CronJob>>>;
/// Thread-safe set of registered handler identifiers.
pub type HandlerRegistry = Arc<HashSet<String>>;

/// Combined Axum application state holding the job store, handler registry, and uptime.
#[derive(Clone)]
pub struct AppState {
    /// Shared cron job store.
    pub store: CronStore,
    /// Registered handler identifiers.
    pub handlers: HandlerRegistry,
    /// Unix timestamp (seconds) when the server started, used for uptime reporting.
    pub uptime_start: u64,
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
/// The `cron` crate requires 7-field internally; 5-field input is normalized before validation.
fn validate_cron(expr: &str) -> Result<(), AppError> {
    let s = expr.trim();
    let normalized = if s.starts_with('@') {
        s.to_string()
    } else {
        let fields: Vec<&str> = s.split_ascii_whitespace().collect();
        match fields.len() {
            5 => format!("0 {} *", fields.join(" ")),
            _ => s.to_string(),
        }
    };
    Schedule::from_str(&normalized)
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
    if let Err(e) = crate::cron_sync::sync_to_crontab(store) {
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
    if let Err(e) = crate::cron_sync::sync_to_crontab(store) {
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
    if let Err(e) = crate::cron_sync::sync_to_crontab(store) {
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

#[cfg(test)]
#[path = "cron_jobs_tests.rs"]
mod cron_jobs_tests;
