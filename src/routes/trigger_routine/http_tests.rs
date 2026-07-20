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
