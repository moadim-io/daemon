#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use tower::ServiceExt;

use super::{api_token_auth, API_TOKEN_ENV, TOKEN_HEADER};

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

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(api_token_auth))
}

#[tokio::test]
async fn missing_token_passes_when_auth_is_not_configured() {
    let _token = EnvGuard::set(None);

    let resp = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn missing_token_is_rejected_when_auth_is_configured() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bad_token_is_rejected_when_auth_is_configured() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::AUTHORIZATION, "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bearer_token_authorizes_request() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::AUTHORIZATION, "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn x_moadim_token_authorizes_request() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(TOKEN_HEADER, "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn malformed_authorization_header_falls_back_to_x_token() {
    let _token = EnvGuard::set(Some("secret"));

    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::AUTHORIZATION, vec![0xff])
                .header(TOKEN_HEADER, "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
