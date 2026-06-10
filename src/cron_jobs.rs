use actix_web::{web, HttpResponse, Responder};
use cron::Schedule;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub handler: String,
    pub metadata: serde_json::Value,
    pub enabled: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

pub type CronStore = Arc<Mutex<HashMap<String, CronJob>>>;

pub fn new_store() -> CronStore {
    Arc::new(Mutex::new(HashMap::new()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn validate_cron(expr: &str) -> Result<(), AppError> {
    Schedule::from_str(expr)
        .map_err(|e| AppError::BadRequest(format!("invalid cron expression: {}", e)))?;
    Ok(())
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateRequest {
    pub schedule: String,
    pub handler: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

fn bool_true() -> bool {
    true
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateRequest {
    pub schedule: Option<String>,
    pub handler: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

// Service layer — shared by HTTP handlers and MCP tools

pub fn svc_create(store: &CronStore, req: CreateRequest) -> Result<CronJob, AppError> {
    validate_cron(&req.schedule)?;
    let now = now_secs();
    let job = CronJob {
        id: Uuid::new_v4().to_string(),
        schedule: req.schedule,
        handler: req.handler,
        metadata: req.metadata,
        enabled: req.enabled,
        created_at: now,
        updated_at: now,
    };
    store.lock().unwrap().insert(job.id.clone(), job.clone());
    Ok(job)
}

pub fn svc_list(store: &CronStore) -> Vec<CronJob> {
    let lock = store.lock().unwrap();
    let mut jobs: Vec<CronJob> = lock.values().cloned().collect();
    jobs.sort_by_key(|j| j.created_at);
    jobs
}

pub fn svc_get(store: &CronStore, id: &str) -> Result<CronJob, AppError> {
    let lock = store.lock().unwrap();
    lock.get(id).cloned().ok_or(AppError::NotFound)
}

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

pub fn svc_delete(store: &CronStore, id: &str) -> Result<(), AppError> {
    let mut lock = store.lock().unwrap();
    lock.remove(id).ok_or(AppError::NotFound).map(|_| ())
}

// HTTP handlers

pub async fn create(
    store: web::Data<CronStore>,
    body: web::Json<CreateRequest>,
) -> AppResult<impl Responder> {
    let job = svc_create(&store, body.into_inner())?;
    Ok(HttpResponse::Created().json(job))
}

pub async fn list(store: web::Data<CronStore>) -> impl Responder {
    HttpResponse::Ok().json(svc_list(&store))
}

pub async fn get(
    store: web::Data<CronStore>,
    path: web::Path<String>,
) -> AppResult<impl Responder> {
    let job = svc_get(&store, &path.into_inner())?;
    Ok(HttpResponse::Ok().json(job))
}

pub async fn update(
    store: web::Data<CronStore>,
    path: web::Path<String>,
    body: web::Json<UpdateRequest>,
) -> AppResult<impl Responder> {
    let job = svc_update(&store, &path.into_inner(), body.into_inner())?;
    Ok(HttpResponse::Ok().json(job))
}

pub async fn delete(
    store: web::Data<CronStore>,
    path: web::Path<String>,
) -> AppResult<impl Responder> {
    svc_delete(&store, &path.into_inner())?;
    Ok(HttpResponse::NoContent().finish())
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
        assert_eq!(svc_get(&store, &job.id).unwrap().handler, "h");
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
}
