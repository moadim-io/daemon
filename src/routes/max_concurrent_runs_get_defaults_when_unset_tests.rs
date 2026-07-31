#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[tokio::test]
async fn max_concurrent_runs_get_defaults_when_unset() {
    let dir =
        std::env::temp_dir().join(format!("moadim-max-runs-default-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/max-concurrent-runs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["value"].as_u64().unwrap(), 0);
    assert!(body["override_value"].is_null());
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn max_concurrent_runs_put_then_get_round_trips() {
    let dir = std::env::temp_dir().join(format!("moadim-max-runs-put-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/max-concurrent-runs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["value"].as_u64().unwrap(), 5);
    assert_eq!(body["override_value"].as_u64().unwrap(), 5);

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/max-concurrent-runs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["value"].as_u64().unwrap(), 5);
    assert_eq!(body["override_value"].as_u64().unwrap(), 5);
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn max_concurrent_runs_put_null_clears_override() {
    let dir = std::env::temp_dir().join(format!("moadim-max-runs-clear-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/max-concurrent-runs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":5}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/max-concurrent-runs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":null}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["value"].as_u64().unwrap(), 0);
    assert!(body["override_value"].is_null());
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn max_concurrent_runs_put_returns_500_on_write_failure() {
    // Place a regular file where the config dir should be so `create_dir_all` fails.
    let dir = std::env::temp_dir().join(format!("moadim-max-runs-fail-{}", uuid::Uuid::new_v4()));
    std::fs::write(&dir, b"").unwrap();
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/max-concurrent-runs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":3}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_file(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}
