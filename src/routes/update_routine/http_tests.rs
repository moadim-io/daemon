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
async fn update_routine_not_found_returns_404() {
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/routines/no-such")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn replace_not_found_returns_404() {
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/routines/no-such")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
