#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt;

use super::{build_app, write_openapi_spec};

// ── openapi spec writer ──────────────────────────────────────────────────────

#[test]
fn write_openapi_spec_writes_json_to_path() {
    let dir = std::env::temp_dir().join(format!("moadim-openapi-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("openapi.json");
    write_openapi_spec(&path);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("openapi"), "spec JSON should be written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_openapi_spec_logs_on_write_failure() {
    // The parent directory exists (so the missing-parent skip doesn't fire), but the target path
    // is itself a directory, so the write fails — exercising the best-effort `log::warn!` branch.
    // The call must not panic.
    let dir = std::env::temp_dir().join(format!("moadim-openapi-fail-{}", uuid::Uuid::new_v4()));
    let unwritable = dir.join("openapi.json");
    std::fs::create_dir_all(&unwritable).unwrap();

    write_openapi_spec(&unwritable);

    assert!(
        unwritable.is_dir(),
        "the write should have failed, leaving the directory untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_openapi_spec_skips_when_parent_dir_is_missing() {
    // Mirrors an installed binary: CARGO_MANIFEST_DIR was baked in at compile time on the build
    // machine and doesn't exist here, so the write must be skipped, not attempted-and-warned.
    let dir = std::env::temp_dir().join(format!("moadim-openapi-missing-{}", uuid::Uuid::new_v4()));
    let path = dir.join("openapi.json");

    write_openapi_spec(&path);

    assert!(
        !path.exists(),
        "should not create the parent dir or the file"
    );
}

// ── build_app / router smoke tests ───────────────────────────────────────────

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
async fn build_app_serves_root() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn build_app_compresses_root_with_gzip() {
    // Issue #399: the ~1.1 MB SPA body should be gzip-compressed when the client advertises
    // support for it.
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(axum::http::header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_ENCODING)
            .unwrap(),
        "gzip"
    );
}

#[tokio::test]
async fn build_app_serves_root_uncompressed_without_accept_encoding() {
    // A client that doesn't advertise gzip support must still get the full identity body.
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp
        .headers()
        .get(axum::http::header::CONTENT_ENCODING)
        .is_none());
}

#[tokio::test]
async fn build_app_serves_root_with_etag() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let etag = resp
        .headers()
        .get(axum::http::header::ETAG)
        .expect("ETag header present")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(etag.starts_with('"') && etag.ends_with('"'));
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CACHE_CONTROL)
            .unwrap(),
        "no-cache"
    );
}

#[tokio::test]
async fn build_app_returns_304_when_if_none_match_matches() {
    // Issue #401: a client that already has the current build sends back the ETag it was given
    // and should get a bodyless 304 instead of re-downloading the ~1.1 MB SPA.
    let app = build_app(crate::routines::new_store());
    let first = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let etag = first
        .headers()
        .get(axum::http::header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(axum::http::header::IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        resp.headers().get(axum::http::header::ETAG).unwrap(),
        etag.as_str()
    );
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(body.is_empty(), "304 response must not carry a body");
}

#[tokio::test]
async fn build_app_serves_root_when_if_none_match_stale() {
    // A stale/mismatched If-None-Match must fall through to the normal 200 body, not a 304.
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(axum::http::header::IF_NONE_MATCH, "\"not-the-real-etag\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn build_app_sets_security_headers_on_ui_and_api() {
    // The whole router carries the security headers (issue #406, hardened further in #551):
    // assert on a representative UI response (the SPA at `/`) and a representative API response
    // (`/api/v1/health`).
    for uri in ["/", "/api/v1/health"] {
        let resp = build_app(crate::routines::new_store())
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            resp.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert_eq!(
            resp.headers().get("content-security-policy").unwrap(),
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
             style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
             font-src 'self' https://fonts.gstatic.com; \
             img-src 'self' data:; \
             connect-src 'self'; \
             base-uri 'none'; \
             form-action 'none'; \
             object-src 'none'; \
             frame-ancestors 'none'"
        );
    }
}

#[tokio::test]
async fn build_app_serves_agents() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let agents: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert!(!agents.is_empty(), "agents list should never be empty");
}

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
async fn build_app_spa_fallback_serves_ui_on_client_routes() {
    // `/routines` (and other client-routed paths) are NOT API endpoints — the API lives under
    // `/api/v1`. Unmatched GETs fall back to the app HTML so the Yew router can resolve the path.
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

#[tokio::test]
async fn router_unknown_api_path_returns_json_404_not_spa() {
    // A path that matches NO route under `/api/v1` (distinct from the nonexistent-id tests,
    // which hit a real handler) must return a JSON 404 — not fall through to the SPA
    // `index.html`/200 via the outer fallback (issue #270).
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let ctype = resp.headers().get(CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("application/json"));
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "not found");
}

#[tokio::test]
async fn router_unknown_api_path_non_get_returns_404() {
    // The fallback covers every method, not just GET: a POST to an unknown `/api/v1` path
    // is a 404 too (issue #270).
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
