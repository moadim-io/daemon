#![allow(clippy::missing_docs_in_private_items)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt;

use super::echo;

#[tokio::test]
async fn echo_returns_message_and_timestamp() {
    let app = Router::new().route("/echo", post(echo));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"message":"hello"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["message"], "hello");
    assert!(json["timestamp"].as_u64().is_some());
}

#[tokio::test]
async fn echo_rejects_invalid_json() {
    let app = Router::new().route("/echo", post(echo));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from("not-json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn echo_rejects_missing_message_field() {
    let app = Router::new().route("/echo", post(echo));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"other":"field"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_system_cron_jobs_returns_json_array() {
    let result = super::list_system_cron_jobs().await;
    // Result is Json<Vec<CronJob>> — verify it doesn't panic
    let _ = result;
}
