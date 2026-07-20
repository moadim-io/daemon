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

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. With no agent TOMLs present, agent validation falls back to the built-in names (so
/// `"claude"` is accepted) while `load_agent_command` finds no config — exercising the trigger
/// "no spawn" path without launching a real agent or writing into the user's real home. Tests in
/// this crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-httptest-{}", uuid::Uuid::new_v4()));
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

// ── routines CRUD lifecycle (covers all routine HTTP handlers) ────────────────

#[tokio::test]
async fn router_routine_full_lifecycle() {
    let _home = TempHome::set();
    let routines = crate::routines::new_store();

    let body = r#"{"schedule":"@daily","title":"Http Routine","agent":"claude","prompt":"p","repositories":[{"repository":"r","branch":"main"}]}"#;
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
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

    // GET list
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // GET one
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // prompt-preview (issue #391): the composed prompt body, computed with no workbench or agent
    // launch.
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}/prompt-preview"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let preview = String::from_utf8(bytes.to_vec()).unwrap();
    // The routine's own prompt body and its declared repository both flow into the preview
    // verbatim (see `compose_prompt`), same as they would in a real run's `prompt.md`.
    assert!(preview.contains("- r (branch main)\n"));
    assert!(preview.trim_end().ends_with('p'));

    // PATCH
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/routines/{id}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"title":"Patched"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // PUT (replace)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/routines/{id}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"prompt":"replaced"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // trigger (records the manual trigger and returns OK)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/routines/{id}/trigger"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // scheduled-trigger (the crontab-invoked path; runs the routine and returns OK)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/routines/{id}/scheduled-trigger"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // logs (empty)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}/logs"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // runs (empty list — no workbench created by this test)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}/runs"))
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
    assert_eq!(runs, serde_json::json!([]));

    // fleet-wide /routines/runs — a static route that must not be shadowed by the dynamic
    // /routines/{id} route registered above.
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines/runs")
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
    assert_eq!(runs, serde_json::json!([]));

    // fleet-wide runs honors a `?limit=` query param.
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines/runs?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // runs/{workbench}/log for a workbench that doesn't exist -> 404
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/routines/{id}/runs/not-a-real-workbench-1/log"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // runs/{workbench}/summary for a workbench that doesn't exist -> 404
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/routines/{id}/runs/not-a-real-workbench-1/summary"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // POST flag
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/routines/{id}/flags"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"type":"bug","description":"broken thing","scope":"general"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let flag: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let filename = flag["filename"].as_str().unwrap().to_string();

    // GET flags
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}/flags"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let flags: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(flags.as_array().unwrap().len(), 1);

    // DELETE flag (resolve)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/routines/{id}/flags/{filename}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // DELETE
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/routines/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!crate::paths::routine_dir(&id).exists());
}

#[tokio::test]
async fn router_flag_not_found_paths() {
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines/no-such/flags")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/routines/no-such/flags/bug-1.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_routine_not_found_paths() {
    for (method, suffix) in [
        ("GET", ""),
        ("DELETE", ""),
        ("POST", "/trigger"),
        ("POST", "/scheduled-trigger"),
        ("GET", "/prompt-preview"),
        ("GET", "/logs"),
        ("GET", "/runs"),
        ("GET", "/runs/some-workbench-1/log"),
        ("GET", "/runs/some-workbench-1/summary"),
    ] {
        let resp = build_app(crate::routines::new_store())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/api/v1/routines/no-such{suffix}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method} {suffix}");
    }

    // PATCH nonexistent
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/routines/no-such")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"title":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
