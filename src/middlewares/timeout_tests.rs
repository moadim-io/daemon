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
use std::time::Duration;
use tower::ServiceExt;

use super::request_timeout;

fn app(timeout: Duration, handler_delay: Duration) -> Router {
    Router::new()
        .route(
            "/",
            get(move || async move {
                tokio::time::sleep(handler_delay).await;
                "ok"
            }),
        )
        .layer(middleware::from_fn(request_timeout(timeout)))
}

#[tokio::test]
async fn a_slow_handler_is_aborted_with_408() {
    let resp = app(Duration::from_millis(20), Duration::from_millis(200))
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn a_handler_within_the_deadline_is_unaffected() {
    let resp = app(Duration::from_millis(200), Duration::from_millis(1))
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
