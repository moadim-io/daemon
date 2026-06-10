use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub uptime_secs: u64,
    pub running: bool,
}

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

async fn health(state: web::Data<AppState>) -> impl Responder {
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

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Server is running")
}

pub async fn run() -> std::io::Result<()> {
    let addr = "127.0.0.1:8080";
    let state = web::Data::new(AppState::new());

    println!("Starting server on http://{}", addr);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
    })
    .bind(addr)?
    .run()
    .await
}
