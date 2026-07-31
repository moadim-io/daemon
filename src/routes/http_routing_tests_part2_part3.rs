#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
    assert!(preview.contains("- ./r — r (branch main)\n"));
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

    // move (explicit filesystem location change)
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/routines/{id}/move"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"folder":"http/folder","slug":"moved-routine"}"#,
                ))
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
