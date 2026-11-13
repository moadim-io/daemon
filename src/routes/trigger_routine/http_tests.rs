#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::routes::http::build_app;

#[tokio::test]
async fn trigger_routine_not_found_returns_404() {
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines/no-such/trigger")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn trigger_routine_with_json_body_not_found_returns_404() {
    // A JSON body runs the `override_system_power_saving` extraction closure in the handler.
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines/no-such/trigger")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"override_system_power_saving":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
