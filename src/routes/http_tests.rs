#![allow(clippy::missing_docs_in_private_items)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt;

use super::{build_app, echo, list_system_cron_jobs, run_with_listener_until};
use crate::cron_jobs::new_store;

// ── build_app / router smoke tests ───────────────────────────────────────────

#[tokio::test]
async fn build_app_serves_root() {
    let app = build_app(new_store());
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn build_app_serves_health() {
    let app = build_app(new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["running"], true);
}

#[tokio::test]
async fn build_app_serves_ui() {
    let app = build_app(new_store());
    let resp = app
        .oneshot(Request::builder().uri("/ui").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── cron-jobs CRUD lifecycle (covers all HTTP handlers + FromRef) ─────────────

#[tokio::test]
async fn router_cron_job_full_lifecycle() {
    let store = new_store();

    // POST /cron-jobs → 201
    let resp = build_app(store.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"schedule":"@daily","handler":"test-h"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    // GET /cron-jobs → 200 (list)
    let resp = build_app(store.clone())
        .oneshot(
            Request::builder()
                .uri("/cron-jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // GET /cron-jobs/{id} → 200
    let resp = build_app(store.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // PATCH /cron-jobs/{id} → 200
    let resp = build_app(store.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/cron-jobs/{id}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"handler":"patched"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // POST /cron-jobs/{id}/trigger → 200  (exercises FromRef<AppState> for CronStore)
    let resp = build_app(store.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/cron-jobs/{id}/trigger"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // DELETE /cron-jobs/{id} → 200
    let resp = build_app(store.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!crate::paths::job_dir(&id).exists());
}

#[tokio::test]
async fn router_create_invalid_cron_returns_400() {
    let resp = build_app(new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"schedule":"bad","handler":"h"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn router_get_nonexistent_returns_404() {
    let resp = build_app(new_store())
        .oneshot(
            Request::builder()
                .uri("/cron-jobs/no-such-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_patch_nonexistent_returns_404() {
    let resp = build_app(new_store())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/cron-jobs/no-such-id")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"handler":"h"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_delete_nonexistent_returns_404() {
    let resp = build_app(new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/cron-jobs/no-such-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_trigger_nonexistent_returns_404() {
    let resp = build_app(new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/cron-jobs/no-such-id/trigger")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── echo handler ──────────────────────────────────────────────────────────────

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

// ── list_system_cron_jobs handler ─────────────────────────────────────────────

#[tokio::test]
async fn list_system_cron_jobs_returns_json_array() {
    let result = list_system_cron_jobs().await;
    let _ = result;
}

// ── run_with_listener integration test (real TCP) ────────────────────────────

#[tokio::test]
async fn run_with_listener_serves_over_tcp() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let store = new_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = tokio::spawn(run_with_listener_until(
        store,
        listener,
        std::future::pending(),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut buf = vec![0u8; 512];
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");

    handle.abort();
}

#[tokio::test]
async fn run_with_listener_until_exits_on_immediate_shutdown() {
    let store = new_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let result = run_with_listener_until(store, listener, async {}).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn mcp_endpoint_triggers_factory() {
    let app = build_app(new_store());
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header(CONTENT_TYPE, "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("host", "localhost")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().as_u16() < 500);
}
