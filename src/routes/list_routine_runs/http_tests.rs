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
async fn list_routine_runs_not_found_returns_404() {
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines/no-such/runs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh temp home for the duration of one test. Tests in this
/// crate run single-threaded, so the env mutation is safe; the caller removes the directory.
fn isolated_home() -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("moadim-http-runs-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    home
}

#[tokio::test]
async fn list_routine_runs_reports_watchdog_kill_as_killed_with_reason() {
    use crate::test_fixtures::routine_fixture;

    let home = isolated_home();
    let title = "Http Killed Runs ZZQ";
    let slug = crate::routines::slugify(title);
    let store = crate::routines::new_store();
    store.lock().unwrap().insert(
        "http-killed".into(),
        routine_fixture("http-killed", title).build(),
    );
    let workbench = crate::paths::workbenches_dir().join(format!("{slug}-1000"));
    std::fs::create_dir_all(&workbench).unwrap();
    std::fs::write(workbench.join("exit_code"), "killed\n").unwrap();

    let resp = build_app(store)
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines/http-killed/runs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let runs: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(runs[0]["status"], "killed");
    assert_eq!(runs[0]["kill_reason"], "max_runtime");
    assert_eq!(runs[0]["exit_code"], serde_json::Value::Null);

    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&home);
}
