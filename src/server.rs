use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use crate::cron_jobs;

/// Server application state shared across handlers.
pub struct AppState {
    pub uptime_start: u64,
    pub running: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            uptime_start: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            running: AtomicBool::new(true),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Health status payload.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub uptime_secs: u64,
    pub running: bool,
}

/// GET / — index page.
pub async fn index() -> impl Responder {
    HttpResponse::Ok().body("Moadim server is running")
}

/// GET /health — health check with uptime tracking.
pub async fn health(state: web::Data<AppState>) -> impl Responder {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - state.uptime_start;

    HttpResponse::Ok().json(HealthResponse {
        status: "ok",
        uptime_secs: secs,
        running: state.running.load(Ordering::Relaxed),
    })
}

/// POST /echo — echoes the request body back with a timestamp.
pub async fn echo(body: web::Bytes) -> Result<impl Responder, actix_web::error::Error> {
    #[derive(serde::Deserialize)]
    struct EchoRequest {
        message: String,
    }

    #[derive(Serialize)]
    struct EchoResponse {
        message: String,
        timestamp: u64,
    }

    let parsed: EchoRequest = serde_json::from_slice(&body)
        .map_err(|e| actix_web::error::ErrorBadRequest(e.to_string()))?;

    Ok(HttpResponse::Ok().json(EchoResponse {
        message: parsed.message,
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }))
}

/// Start the HTTP server on 127.0.0.1:8080.
pub async fn run() -> std::io::Result<()> {
    let addr = "127.0.0.1:8080";
    let state = web::Data::new(AppState::new());
    let cron_store = web::Data::new(cron_jobs::new_store());

    println!("Starting server on http://{}", addr);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(cron_store.clone())
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
            .route("/echo", web::post().to(echo))
            .route("/cron-jobs", web::post().to(cron_jobs::create))
            .route("/cron-jobs", web::get().to(cron_jobs::list))
            .route("/cron-jobs/{id}", web::get().to(cron_jobs::get))
            .route("/cron-jobs/{id}", web::put().to(cron_jobs::update))
            .route("/cron-jobs/{id}", web::patch().to(cron_jobs::update))
            .route("/cron-jobs/{id}", web::delete().to(cron_jobs::delete))
    })
    .bind(addr)?
    .run()
    .await
}
