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
async fn router_flag_create_rejects_bad_scope() {
    let routines = crate::routines::new_store();
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schedule":"@daily","title":"Flag Scope Routine","agent":"claude","prompt":"p"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let id = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/routines/{id}/flags"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"type":"bug","description":"d","scope":"nowhere"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
