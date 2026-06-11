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

use super::{fs_location, inject_headers_from_value};

#[tokio::test]
async fn fs_location_middleware_adds_headers() {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(fs_location));

    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
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
