#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::{build_app, SucceedingCronShim, TempHome};

// ── Global lock endpoints ─────────────────────────────────────────────────────

#[tokio::test]
async fn get_lock_status_returns_unlocked_by_default() {
    let _home = TempHome::set();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines/lock")
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
    assert_eq!(json["shared"], false);
    assert_eq!(json["local"], false);
    assert_eq!(json["locked"], false);
}

#[tokio::test]
async fn lock_route_creates_sentinel_and_returns_status() {
    let _home = TempHome::set();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines/lock")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"scope":"shared"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["shared"], true);
    assert_eq!(json["locked"], true);
    // Cleanup.
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, false).unwrap();
}

#[tokio::test]
async fn lock_route_unknown_scope_is_bad_request() {
    let _home = TempHome::set();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines/lock")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"scope":"global"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unlock_route_removes_sentinel_and_returns_status() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, true).unwrap();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/routines/lock?scope=local")
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
    assert_eq!(json["local"], false);
    assert_eq!(json["locked"], false);
}

#[tokio::test]
async fn unlock_route_all_removes_both_sentinels() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, true).unwrap();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, true).unwrap();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/routines/lock?scope=all")
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
    assert_eq!(json["shared"], false);
    assert_eq!(json["local"], false);
    assert_eq!(json["locked"], false);
}

#[tokio::test]
async fn lock_route_sync_success_path() {
    // Covers the fall-through `}` of `if let Err(sync_err)` in the lock handler when sync passes.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines/lock")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"scope":"local"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, false).unwrap();
}

#[tokio::test]
async fn unlock_route_sync_success_path() {
    // Covers the fall-through `}` of `if let Err(sync_err)` in the unlock handler when sync passes.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/routines/lock?scope=all")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn unlock_route_unknown_scope_is_bad_request() {
    let _home = TempHome::set();
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/routines/lock?scope=everything")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
