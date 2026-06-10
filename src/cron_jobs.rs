use actix_web::{web, HttpResponse, Responder};
use cron::Schedule;
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

fn validate_cron(expr: &str) -> Result<(), AppError> {
    Schedule::from_str(expr)
        .map_err(|e| AppError::BadRequest(format!("invalid cron expression: {}", e)))?;
    Ok(())
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub schedule: Option<String>,
    pub handler: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

/// POST /cron-jobs
pub async fn create(
    store: web::Data<CronStore>,
    body: web::Json<CreateRequest>,
) -> AppResult<impl Responder> {
    validate_cron(&body.schedule)?;
    let now = now_secs();
    let job = CronJob {
        id: Uuid::new_v4().to_string(),
        schedule: body.schedule.clone(),
        handler: body.handler.clone(),
        metadata: body.metadata.clone(),
        enabled: body.enabled,
        created_at: now,
        updated_at: now,
    };
    store.lock().unwrap().insert(job.id.clone(), job.clone());
    Ok(HttpResponse::Created().json(job))
}

/// GET /cron-jobs
pub async fn list(store: web::Data<CronStore>) -> impl Responder {
    let lock = store.lock().unwrap();
    let mut jobs: Vec<&CronJob> = lock.values().collect();
    jobs.sort_by_key(|j| j.created_at);
    HttpResponse::Ok().json(jobs)
}

/// GET /cron-jobs/{id}
pub async fn get(
    store: web::Data<CronStore>,
    path: web::Path<String>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let lock = store.lock().unwrap();
    let job = lock.get(&id).ok_or(AppError::NotFound)?;
    Ok(HttpResponse::Ok().json(job))
}

/// PUT /cron-jobs/{id} and PATCH /cron-jobs/{id}
pub async fn update(
    store: web::Data<CronStore>,
    path: web::Path<String>,
    body: web::Json<UpdateRequest>,
) -> AppResult<impl Responder> {
    if let Some(ref sched) = body.schedule {
        validate_cron(sched)?;
    }
    let id = path.into_inner();
    let mut lock = store.lock().unwrap();
    let job = lock.get_mut(&id).ok_or(AppError::NotFound)?;
    if let Some(ref s) = body.schedule {
        job.schedule = s.clone();
    }
    if let Some(ref h) = body.handler {
        job.handler = h.clone();
    }
    if let Some(ref m) = body.metadata {
        job.metadata = m.clone();
    }
    if let Some(e) = body.enabled {
        job.enabled = e;
    }
    job.updated_at = now_secs();
    Ok(HttpResponse::Ok().json(job.clone()))
}

/// DELETE /cron-jobs/{id}
pub async fn delete(
    store: web::Data<CronStore>,
    path: web::Path<String>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let mut lock = store.lock().unwrap();
    lock.remove(&id).ok_or(AppError::NotFound)?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> web::Data<CronStore> {
        web::Data::new(new_store())
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
    fn store_insert_and_get() {
        let store = new_store();
        let job = CronJob {
            id: "x".to_string(),
            schedule: "@hourly".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            enabled: true,
            created_at: 0,
            updated_at: 0,
        };
        store.lock().unwrap().insert("x".to_string(), job);
        assert!(store.lock().unwrap().contains_key("x"));
    }
}
