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

use super::{allowed_hosts, host_validation};

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .route("/", post(|| async { "ok" }))
        .layer(middleware::from_fn(host_validation(vec![
            "example.com".to_string(),
            "example.com:5784".to_string(),
        ])))
}

#[tokio::test]
async fn allowed_host_passes() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn disallowed_host_is_rejected() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_host_header_passes() {
    // No real HTTP client omits `Host`; this mirrors how in-process test requests are built
    // elsewhere in the suite and must not be rejected (see the `host_validation` doc comment).
    let resp = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn cross_origin_post_is_rejected() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header(header::HOST, "example.com")
                .header(header::ORIGIN, "http://attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn same_origin_post_passes() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header(header::HOST, "example.com")
                .header(header::ORIGIN, "http://example.com:5784")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn missing_origin_on_post_passes() {
    // No `Origin` header means a non-browser client (curl, the CLI, MCP) with nothing to forge.
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header(header::HOST, "example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn cross_origin_get_is_not_rejected() {
    // Origin is only enforced on state-changing methods; a GET can't mutate anything.
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "example.com")
                .header(header::ORIGIN, "http://attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded (`RUST_TEST_THREADS=1`, see
        // `.cargo/config.toml`), so no other thread observes the env in between.
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution, see `set` above.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

#[test]
fn allowed_hosts_includes_loopback_defaults() {
    let hosts = allowed_hosts();
    assert!(hosts.iter().any(|host| host == "localhost"));
    assert!(hosts.iter().any(|host| host == "127.0.0.1"));
    assert!(hosts.iter().any(|host| host == "[::1]"));
}

#[test]
fn allowed_hosts_extends_from_env_var() {
    let _guard = EnvGuard::set(
        "MOADIM_ALLOWED_HOSTS",
        "reverse-proxy.internal, other.host:8080",
    );
    let hosts = allowed_hosts();
    assert!(hosts.iter().any(|host| host == "reverse-proxy.internal"));
    assert!(hosts.iter().any(|host| host == "other.host:8080"));
}
