#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

use crate::middlewares::api_token::{API_TOKEN_ENV, TOKEN_HEADER};
use crate::routes::http::build_app;

struct EnvGuard {
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(value: Option<&str>) -> Self {
        let previous = std::env::var_os(API_TOKEN_ENV);
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            match value {
                Some(value) => std::env::set_var(API_TOKEN_ENV, value),
                None => std::env::remove_var(API_TOKEN_ENV),
            }
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(API_TOKEN_ENV, value),
                None => std::env::remove_var(API_TOKEN_ENV),
            }
        }
    }
}

#[tokio::test]
async fn loopback_default_no_token_keeps_rest_api_open() {
    let _token = EnvGuard::set(None);

    let resp = build_app(crate::routines::new_store())
        .oneshot(Request::builder().uri("/api/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn configured_token_rejects_missing_rest_credentials() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = build_app(crate::routines::new_store())
        .oneshot(Request::builder().uri("/api/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn configured_token_accepts_bearer_rest_credentials() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header(header::AUTHORIZATION, "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn configured_token_accepts_x_moadim_token_rest_credentials() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header(TOKEN_HEADER, "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn configured_token_rejects_missing_mcp_credentials() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = build_app(crate::routines::new_store())
        .oneshot(Request::builder().method("POST").uri("/mcp").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn configured_token_does_not_block_served_ui_shell() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = build_app(crate::routines::new_store())
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
