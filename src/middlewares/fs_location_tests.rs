#![allow(clippy::missing_docs_in_private_items)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    response::Response,
    routing::get,
    Router,
};
use tower::ServiceExt;

use super::{fs_location, inject_headers_from_value, DEBUG_FS_HEADERS_ENV};

async fn fs_location_response() -> Response {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(fs_location));

    app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap()
}

/// The middleware is the sole reader of `DEBUG_FS_HEADERS_ENV`, but env vars are
/// process-global, so the default-off and opt-in cases share one test to avoid
/// racing each other under parallel test execution.
#[tokio::test]
async fn fs_location_headers_off_by_default_on_when_opted_in() {
    // Default: no env var → no x-server-* headers leak on a normal response.
    std::env::remove_var(DEBUG_FS_HEADERS_ENV);
    let resp = fs_location_response().await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("x-server-root").is_none());
    assert!(resp.headers().get("x-server-exe-dir").is_none());

    // Opt-in: a truthy env var re-enables the debug headers.
    std::env::set_var(DEBUG_FS_HEADERS_ENV, "1");
    let resp = fs_location_response().await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("x-server-root").is_some());

    std::env::remove_var(DEBUG_FS_HEADERS_ENV);
}

#[test]
fn inject_headers_from_value_non_object_returns_unchanged() {
    let res = Response::new(Body::empty());
    let res = inject_headers_from_value(res, serde_json::json!("not-an-object"));
    assert!(res.headers().is_empty());
}

#[test]
fn inject_headers_from_value_null_value_skipped() {
    let res = Response::new(Body::empty());
    let res = inject_headers_from_value(res, serde_json::json!({"server_root": null}));
    assert!(res.headers().get("x-server-root").is_none());
}

#[test]
fn inject_headers_from_value_sets_string_value() {
    let res = Response::new(Body::empty());
    let res = inject_headers_from_value(res, serde_json::json!({"server_root": "/tmp/test"}));
    assert_eq!(res.headers().get("x-server-root").unwrap(), "/tmp/test");
}

#[test]
fn inject_headers_from_value_invalid_header_value_skipped() {
    let res = Response::new(Body::empty());
    // Header values must be printable ASCII; newline is invalid
    let res = inject_headers_from_value(
        res,
        serde_json::json!({"server_root": "path\nwith\nnewline"}),
    );
    assert!(res.headers().get("x-server-root").is_none());
}
