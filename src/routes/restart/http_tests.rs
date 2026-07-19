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

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop, so the detached restart helper's log file doesn't land in the real home. Tests in this
/// crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-restarthttptest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
    }
}

#[tokio::test]
async fn build_app_restart_route_acknowledges() {
    // The route spawns a detached `current_exe --background` helper; under the test harness that exe
    // is the test binary, which rejects `--background` and exits at once, so no real server starts.
    // TempHome keeps the helper's log file out of the real home.
    let _home = TempHome::set();
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/restart")
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
    assert_eq!(json["status"], "restarting");
    assert!(json["helper_pid"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn build_app_restart_route_returns_500_when_spawn_fails() {
    // Cover the `map_err(|_| AppError::Internal)?` branch in the restart handler: make
    // spawn_restart() fail by placing a regular file at the `.config` component of the home path so
    // create_dir_all() for the daemon log directory errors out.
    let dir = std::env::temp_dir().join(format!("moadim-restart-fail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    // A regular file at `.config` blocks create_dir_all(".config/moadim") inside spawn_detached_with.
    std::fs::write(dir.join(".config"), b"blocker").unwrap();
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }

    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/restart")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // SAFETY: cleanup before asserting so the env var is always removed.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        resp.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "restart route should return 500 when spawn_restart fails"
    );
}
