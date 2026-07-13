#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::health;
use crate::routes::http::{build_app, AppState};
use crate::utils::time::now_secs;

#[tokio::test]
async fn build_app_serves_health() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
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
    // The dependencies section reports tmux presence so the UI/CLI can flag a missing dependency.
    assert!(
        json["dependencies"]["tmux"].is_boolean(),
        "health payload should carry a boolean dependencies.tmux flag, got: {json}"
    );
    // Likewise for python3, which the built-in `claude` agent's setup step depends on (#404).
    assert!(
        json["dependencies"]["python3"].is_boolean(),
        "health payload should carry a boolean dependencies.python3 flag, got: {json}"
    );
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    // The resolved machine name is surfaced so clients can identify which daemon answered.
    assert!(
        json["machine"].is_string() && !json["machine"].as_str().unwrap().is_empty(),
        "health payload should carry a non-empty machine name, got: {json}"
    );
    // Filesystem locations, mirroring the MCP health tool.
    assert!(
        json["server_root"].is_string() || json["server_root"].is_null(),
        "health payload should carry server_root (string or null), got: {json}"
    );
}

#[tokio::test]
async fn health_uptime_clamps_to_zero_on_backward_clock_skew() {
    // A `uptime_start` in the future models the wall clock jumping backward
    // after the server started. The old `now_secs() - uptime_start` would
    // underflow; saturating_sub must clamp uptime to 0 instead.
    let state = AppState {
        routines: crate::routines::new_store(),
        routines_dir: crate::paths::routines_dir(),
        uptime_start: now_secs() + 10_000,
        shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
    };
    let resp = health(axum::extract::State(state)).await;
    assert_eq!(resp.0.uptime_secs, 0);
    assert_eq!(resp.0.status, "ok");
    assert!(resp.0.running);
}
