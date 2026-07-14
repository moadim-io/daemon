#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt;

use super::build_app;

// ── machine / user-prompt settings route tests ───────────────────────────────

#[tokio::test]
async fn put_machine_updates_name() {
    let dir = std::env::temp_dir().join(format!("moadim-machine-put-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/machine")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"my-box"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["name"].as_str().unwrap(), "my-box");
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn put_machine_rejects_empty_name() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/machine")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn put_machine_returns_500_on_write_failure() {
    // Place a regular file where the config dir should be so `create_dir_all` fails.
    let dir = std::env::temp_dir().join(format!("moadim-machine-fail-{}", uuid::Uuid::new_v4()));
    std::fs::write(&dir, b"").unwrap();
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/machine")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"new-name"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_file(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn build_app_serves_machine() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/machine")
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
    assert!(body["name"].is_string() && !body["name"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn user_prompt_empty_when_unset() {
    let dir =
        std::env::temp_dir().join(format!("moadim-user-prompt-empty-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/user-prompt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(bytes, "".as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn user_prompt_get_returns_500_on_non_not_found_read_error() {
    // A directory in place of the file makes `read_to_string` fail with something other than
    // `NotFound` (e.g. `IsADirectory`), exercising the `Err(_)` arm distinct from the "unset"
    // (`NotFound` -> empty string) case.
    let dir =
        std::env::temp_dir().join(format!("moadim-user-prompt-isdir-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    std::fs::create_dir_all(crate::paths::user_prompt_path()).unwrap();
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/user-prompt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn user_prompt_put_then_get_round_trips() {
    let dir = std::env::temp_dir().join(format!("moadim-user-prompt-put-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/user-prompt")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"always be terse"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/user-prompt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(bytes, "always be terse".as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn user_prompt_put_returns_500_on_write_failure() {
    // Place a regular file where the config dir should be so `create_dir_all` fails.
    let dir =
        std::env::temp_dir().join(format!("moadim-user-prompt-fail-{}", uuid::Uuid::new_v4()));
    std::fs::write(&dir, b"").unwrap();
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/user-prompt")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_file(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn user_prompt_put_returns_500_when_target_path_is_a_directory() {
    // `create_private_dir_all(parent)` succeeds here (the parent is a normal, writable
    // directory); a directory in place of the prompt file itself makes the *second* fallible
    // call, `std::fs::write(&path, ...)`, fail — the distinct `?` this test exercises versus
    // `user_prompt_put_returns_500_on_write_failure` above, which only ever reaches the first.
    let dir = std::env::temp_dir().join(format!(
        "moadim-user-prompt-targetdir-{}",
        uuid::Uuid::new_v4()
    ));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    std::fs::create_dir_all(crate::paths::user_prompt_path()).unwrap();
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/user-prompt")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

// ── max-concurrent-runs settings route tests (issue #1155) ───────────────────

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
