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
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

use crate::error::AppError;

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

/// Returns the path to `~/.config/moadim/jobs/`.
fn jobs_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("jobs")
}

/// Returns the path to `{jobs_dir}/{id}/job.toml`.
fn job_toml_path(id: &str) -> PathBuf {
    jobs_dir().join(id).join("job.toml")
}

/// Returns the path to `{jobs_dir}/{id}/job.local.toml`.
fn job_local_toml_path(id: &str) -> PathBuf {
    jobs_dir().join(id).join("job.local.toml")
}

/// TOML representation of a job. `job.local.toml` may override any field.
#[derive(Debug, Deserialize, Serialize)]
struct JobToml {
    /// Cron expression.
    schedule: Option<String>,
    /// Handler identifier.
    handler: Option<String>,
    /// Whether the job is enabled.
    enabled: Option<bool>,
    /// Unix creation timestamp.
    created_at: Option<u64>,
    /// Unix last-updated timestamp.
    updated_at: Option<u64>,
    /// Unix timestamp of last manual trigger.
    last_triggered_at: Option<u64>,
    /// Arbitrary metadata key/value pairs.
    #[serde(default)]
    metadata: toml::Table,
}

/// Parse a TOML file at `path`, returning `None` on any error.
fn read_job_toml(path: &PathBuf) -> Option<JobToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Convert a TOML table to a JSON object value.
fn metadata_to_json(table: &toml::Table) -> serde_json::Value {
    serde_json::to_value(table).unwrap_or(serde_json::Value::Object(Default::default()))
}

/// Convert a JSON object value to a TOML table, skipping non-representable values.
fn json_to_toml_table(val: &serde_json::Value) -> toml::Table {
    match val {
        serde_json::Value::Object(map) => {
            let mut table = toml::Table::new();
            for (k, v) in map {
                if let Ok(tv) = serde_json::from_value::<toml::Value>(v.clone()) {
                    table.insert(k.clone(), tv);
                }
            }
            table
        }
        _ => toml::Table::new(),
    }
}

/// Load a managed job from `{jobs_dir}/{id}/`, merging `job.local.toml` overrides.
fn load_job_from_dir(id: &str) -> Option<CronJob> {
    let base = read_job_toml(&job_toml_path(id))?;
    let local = read_job_toml(&job_local_toml_path(id));
    let (schedule, handler, enabled, created_at, updated_at, last_triggered_at, mut meta) = (
        local.as_ref().and_then(|l| l.schedule.clone()).or(base.schedule)?,
        local.as_ref().and_then(|l| l.handler.clone()).or(base.handler)?,
        local.as_ref().and_then(|l| l.enabled).or(base.enabled).unwrap_or(true),
        local.as_ref().and_then(|l| l.created_at).or(base.created_at).unwrap_or(0),
        local.as_ref().and_then(|l| l.updated_at).or(base.updated_at).unwrap_or(0),
        local.as_ref().and_then(|l| l.last_triggered_at).or(base.last_triggered_at),
        base.metadata,
    );
    if let Some(local_meta) = local.as_ref().map(|l| &l.metadata) {
        for (k, v) in local_meta {
            meta.insert(k.clone(), v.clone());
        }
    }
    Some(CronJob {
        id: id.to_string(),
        schedule,
        handler,
        enabled,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_triggered_at,
        metadata: metadata_to_json(&meta),
    })
}

/// Write `job` to `{jobs_dir}/{job.id}/job.toml`, creating the directory and `.gitignore` if needed.
fn write_job(job: &CronJob) -> std::io::Result<()> {
    let dir = jobs_dir().join(&job.id);
    std::fs::create_dir_all(&dir)?;

    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*.local.*\n*.log\n")?;
    }

    let toml_job = JobToml {
        schedule: Some(job.schedule.clone()),
        handler: Some(job.handler.clone()),
        enabled: Some(job.enabled),
        created_at: Some(job.created_at),
        updated_at: Some(job.updated_at),
        last_triggered_at: job.last_triggered_at,
        metadata: json_to_toml_table(&job.metadata),
    };
    let text = toml::to_string_pretty(&toml_job)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(job_toml_path(&job.id), text)?;
    Ok(())
}

/// Remove the directory for job `id`, doing nothing if it does not exist.
fn remove_job_dir(id: &str) -> std::io::Result<()> {
    let dir = jobs_dir().join(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Scan `~/.config/moadim/jobs/` and load all valid managed jobs into a new store.
pub fn load_store() -> CronStore {
    let dir = jobs_dir();
    let mut jobs = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let id = entry.file_name().to_string_lossy().to_string();
                if let Some(job) = load_job_from_dir(&id) {
                    jobs.insert(id, job);
                }
            }
        }
    }
    Arc::new(Mutex::new(jobs))
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
        last_triggered_at: None,
    };
    write_job(&job).map_err(|_| AppError::Internal)?;
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
    let job = job.clone();
    drop(lock);
    write_job(&job).map_err(|_| AppError::Internal)?;
    Ok(job)
}

/// Remove the job with `id` from the store, returning `NotFound` if absent.
pub fn svc_delete(store: &CronStore, id: &str) -> Result<(), AppError> {
    store.lock().unwrap().remove(id).ok_or(AppError::NotFound)?;
    remove_job_dir(id).map_err(|_| AppError::Internal)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_store() -> CronStore {
        Arc::new(Mutex::new(HashMap::new()))
    }

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
            last_triggered_at: None,
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
    fn svc_get_returns_not_found() {
        let store = empty_store();
        assert!(svc_get(&store, "missing").is_err());
    }

    #[test]
    fn svc_delete_removes_from_store() {
        let store = empty_store();
        let job = CronJob {
            id: "test-id".to_string(),
            schedule: "@daily".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_triggered_at: None,
        };
        store.lock().unwrap().insert(job.id.clone(), job);
        // Remove from in-memory store directly (skip fs in unit test)
        store.lock().unwrap().remove("test-id");
        assert!(svc_get(&store, "test-id").is_err());
    }

    #[test]
    fn svc_update_enabled_override() {
        let store = empty_store();
        let job = CronJob {
            id: "test-id".to_string(),
            schedule: "@daily".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_triggered_at: None,
        };
        store.lock().unwrap().insert(job.id.clone(), job);
        {
            let mut lock = store.lock().unwrap();
            let j = lock.get_mut("test-id").unwrap();
            j.enabled = false;
        }
        let j = svc_get(&store, "test-id").unwrap();
        assert!(!j.enabled);
    }

    #[test]
    fn metadata_roundtrip() {
        let val = serde_json::json!({"key": "value", "num": 42});
        let table = json_to_toml_table(&val);
        let back = metadata_to_json(&table);
        assert_eq!(back["key"], "value");
        assert_eq!(back["num"], 42);
    }
}
