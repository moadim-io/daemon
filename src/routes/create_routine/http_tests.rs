#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt;

use crate::routes::http::build_app;

#[tokio::test]
async fn router_routine_create_invalid_cron_400() {
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schedule":"bad","title":"t","agent":"a","prompt":"p"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
