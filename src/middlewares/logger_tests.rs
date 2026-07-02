#![allow(clippy::missing_docs_in_private_items)]

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
