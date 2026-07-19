#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use tower::ServiceExt;

use super::logger;

#[tokio::test]
async fn logger_passes_200_response_through() {
    log::set_max_level(log::LevelFilter::Trace);
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn logger_passes_health_response_through_at_debug() {
    // `/api/v1/health` is logged at debug instead of info; exercise that branch
    // so the health-poll path is covered and still forwards the response intact.
    log::set_max_level(log::LevelFilter::Trace);
    let app = Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn logger_passes_404_response_through() {
    let app = Router::new()
        .route("/exists", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn logger_stamps_a_generated_request_id_header() {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let id = resp
        .headers()
        .get("x-request-id")
        .expect("x-request-id header set")
        .to_str()
        .unwrap();
    assert_eq!(id.len(), 8, "generated id is an 8-hex-digit counter value");
}

#[tokio::test]
async fn logger_reuses_an_inbound_request_id() {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("x-request-id", "caller-supplied-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.headers().get("x-request-id").unwrap(),
        "caller-supplied-id"
    );
}

#[tokio::test]
async fn logger_generates_an_id_when_inbound_request_id_is_empty() {
    // An empty `x-request-id` (e.g. a proxy that sends the header with no
    // value) must not be echoed back verbatim — it should be treated the same
    // as a missing header and get a freshly generated id instead.
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("x-request-id", "")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let id = resp
        .headers()
        .get("x-request-id")
        .expect("x-request-id header set")
        .to_str()
        .unwrap();
    assert_eq!(id.len(), 8, "generated id is an 8-hex-digit counter value");
}

#[tokio::test]
async fn logger_gives_concurrent_requests_distinct_generated_ids() {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(logger));

    let resp_a = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let resp_b = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let id_a = resp_a
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();
    let id_b = resp_b
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();
    assert_ne!(id_a, id_b);
}
