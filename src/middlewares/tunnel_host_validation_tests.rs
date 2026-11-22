#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware,
    routing::{get, post},
    Router,
};
use tower::ServiceExt;

use super::host_validation;

const TUNNEL_HOST: &str = "fancy-dev-run.trycloudflare.com";

/// App guarded by `example.com` exact-match plus the `trycloudflare.com` tunnel suffix.
fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .route("/", post(|| async { "ok" }))
        .layer(middleware::from_fn(host_validation(
            vec!["localhost".to_string()],
            || async { vec!["trycloudflare.com".to_string()] },
        )))
}

/// App with no tunnel suffix configured, as on a machine where the operator opted in to nothing.
fn app_without_tunnel_suffix() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .route("/", post(|| async { "ok" }))
        .layer(middleware::from_fn(host_validation(
            vec!["localhost".to_string()],
            || async { Vec::new() },
        )))
}

async fn send(app: Router, method: &str, host: &str, origin: Option<&str>) -> StatusCode {
    let mut builder = Request::builder()
        .method(method)
        .uri("/")
        .header(header::HOST, host);
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    app.oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn allowed_tunnel_host_passes() {
    let status = send(app(), "GET", TUNNEL_HOST, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_host_still_403_with_tunnel_suffix_configured() {
    let status = send(app(), "GET", "attacker.example", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn bare_tunnel_suffix_host_is_still_403() {
    let status = send(app(), "GET", "trycloudflare.com", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tunnel_host_is_403_when_no_suffix_configured() {
    let status = send(app_without_tunnel_suffix(), "GET", TUNNEL_HOST, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mutation_from_tunnel_origin_passes() {
    let origin = format!("https://{TUNNEL_HOST}");
    let status = send(app(), "POST", TUNNEL_HOST, Some(&origin)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn mutation_from_exact_allowed_origin_passes_on_tunnel_host() {
    let status = send(app(), "POST", TUNNEL_HOST, Some("http://localhost")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn mutation_from_other_tunnel_origin_is_403() {
    // A page on a different quick-tunnel subdomain must not drive mutations on this tunnel.
    let status = send(
        app(),
        "POST",
        TUNNEL_HOST,
        Some("https://attacker-tunnel.trycloudflare.com"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mutation_from_foreign_origin_on_tunnel_host_is_403() {
    let status = send(app(), "DELETE", TUNNEL_HOST, Some("https://evil.example")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mutation_from_tunnel_origin_is_403_without_suffix_configured() {
    let origin = format!("https://{TUNNEL_HOST}");
    let status = send(
        app_without_tunnel_suffix(),
        "POST",
        TUNNEL_HOST,
        Some(&origin),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// App wired to the real `machine.local.toml` loader, as the daemon's `build_app` is.
fn app_with_file_suffixes() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(host_validation(
            vec!["localhost".to_string()],
            crate::machine::tunnel_host_suffixes,
        )))
}

#[tokio::test]
async fn one_label_suffix_in_config_trusts_nothing() {
    use crate::machine::tunnel_host_suffixes_tests::{temp_home, HomeGuard};
    let home = temp_home("one-label");
    let _home = HomeGuard::set(&home);
    let config = crate::paths::machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = [\"com\"]\n").unwrap();
    let status = send(app_with_file_suffixes(), "GET", "foo.com", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn trailing_dot_tunnel_host_is_403() {
    let host = format!("{TUNNEL_HOST}.");
    let status = send(app(), "GET", &host, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn lookalike_and_attacker_suffix_hosts_are_403() {
    for host in [
        "eviltrycloudflare.com",
        "trycloudflare.com.attacker.example",
        "fancy.trycloudflare.com.attacker.example",
    ] {
        let status = send(app(), "GET", host, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "host {host}");
    }
}

#[tokio::test]
async fn mutation_origin_differing_only_in_case_passes() {
    let origin = "https://FANCY-DEV-RUN.TryCloudflare.com";
    let status = send(app(), "POST", TUNNEL_HOST, Some(origin)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn mutation_origin_with_different_port_is_403() {
    let origin = format!("https://{TUNNEL_HOST}:8443");
    let status = send(app(), "POST", TUNNEL_HOST, Some(&origin)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mutation_origin_without_port_when_host_has_port_is_403() {
    let host = format!("{TUNNEL_HOST}:8443");
    let origin = format!("https://{TUNNEL_HOST}");
    let status = send(app(), "POST", &host, Some(&origin)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
