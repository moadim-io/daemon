#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[tokio::test]
async fn build_app_serves_machines() {
    // Seed a routine so the response exercises de-duplication against the implicit
    // local-identity entry.
    let routines = crate::routines::new_store();
    routines.lock().unwrap().insert(
        "r1".to_string(),
        crate::routines::Routine {
            model: None,
            id: "r1".to_string(),
            schedule: "@daily".to_string(),
            schedules: vec![],
            title: "R".to_string(),
            agent: "claude".to_string(),
            prompt: "p".to_string(),
            goal: None,
            repositories: vec![],
            machines: vec!["alpha-box".to_string(), "shared".to_string()],
            tags: vec![],
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_manual_trigger_at: None,
            last_scheduled_trigger_at: None,
            snoozed_until: None,
            skip_runs: None,
            power_saving: false,
            ttl_secs: None,
            max_runtime_secs: None,
            env: std::collections::HashMap::new(),
            auto_disabled_reason: None,
            consecutive_failures: 0,
            failure_threshold: None,
        },
    );
    let resp = build_app(routines)
        .oneshot(
            Request::builder()
                .uri("/api/v1/machines")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let machines: Vec<String> = serde_json::from_slice(&bytes).unwrap();

    let mut expected = vec![
        crate::machine::current_machine(),
        "alpha-box".to_string(),
        "shared".to_string(),
    ];
    expected.sort();
    expected.dedup();
    assert_eq!(machines, expected);
}

#[tokio::test]
async fn build_app_serves_ui_at_root() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("text/html"));
}

#[tokio::test]
async fn build_app_redirects_ui_to_root() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(Request::builder().uri("/ui").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resp.headers().get("location").unwrap(), "/");
}

#[tokio::test]
async fn build_app_redirects_client_to_root() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/client")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resp.headers().get("location").unwrap(), "/");
}

#[tokio::test]
async fn build_app_redirects_client_deep_link_to_root_path() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/client/routines")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resp.headers().get("location").unwrap(), "/routines");
}

#[tokio::test]
async fn build_app_redirects_client_deep_link_preserving_query() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/client/routines?history=abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        resp.headers().get("location").unwrap(),
        "/routines?history=abc"
    );
}

#[tokio::test]
async fn build_app_spa_fallback_serves_ui_on_client_routes() {
    // `/routines` (and other client-routed paths) are NOT API endpoints — the API lives under
    // `/api/v1`. Unmatched GETs fall back to the app HTML so React Router can resolve the path.
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/routines")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("text/html"));
}
include!("http_tests_part4.rs");
