#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

use super::build_app;
use crate::machine::tunnel_host_suffixes_tests::{temp_home, HomeGuard};

const TUNNEL_HOST: &str = "fancy-dev-run.trycloudflare.com";

/// Send `GET /api/v1/health` with `host` through a freshly built app, returning the status.
async fn health_status(host: &str) -> StatusCode {
    let request = Request::builder()
        .uri("/api/v1/health")
        .header(header::HOST, host)
        .body(Body::empty())
        .unwrap();
    build_app(crate::routines::new_store())
        .oneshot(request)
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn built_app_applies_tunnel_host_suffixes_from_config() {
    let home = temp_home("wiring");
    let _home = HomeGuard::set(&home);
    let config = crate::paths::machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();

    std::fs::write(&config, "tunnel_host_suffixes = [\"trycloudflare.com\"]\n").unwrap();
    assert_ne!(health_status(TUNNEL_HOST).await, StatusCode::FORBIDDEN);

    std::fs::remove_file(&config).unwrap();
    assert_eq!(health_status(TUNNEL_HOST).await, StatusCode::FORBIDDEN);
}
