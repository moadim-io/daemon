#![allow(clippy::missing_docs_in_private_items)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt;

use super::{build_app, echo, health, run_with_listener_until, write_openapi_spec};
use crate::cron_jobs::{new_registry, new_store, AppState};
use crate::utils::time::now_secs;

/// Crontab shim that makes sync succeed: `-l` prints the stored content, `-` writes stdin to a
/// temp file. Used to cover the `if let Err(sync_err)` success fall-through in the lock handlers.
struct SucceedingCronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl SucceedingCronShim {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-httpcshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store = base.join("store");
        std::fs::write(&store, "").unwrap();
        let store_display = store.to_string_lossy().into_owned();
        let script = base.join("crontab-ok.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then cat \"$STORE\"; elif [ \"$1\" = \"-\" ]; then cat > \"$STORE\"; fi\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for SucceedingCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(val) => std::env::set_var("MOADIM_CRONTAB_BIN", val),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. With no agent TOMLs present, agent validation falls back to the built-in names (so
/// `"claude"` is accepted) while `load_agent_command` finds no config — exercising the trigger
/// "no spawn" path without launching a real agent or writing into the user's real home. Tests in
/// this crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> TempHome {
        let dir = std::env::temp_dir().join(format!("moadim-httptest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        TempHome
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
    // The target's parent is a regular file, so writing the spec underneath it fails,
    // exercising the best-effort `log::warn!` branch. The call must not panic.
    let dir = std::env::temp_dir().join(format!("moadim-openapi-fail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, "i am a file").unwrap();
    let unwritable = blocker.join("openapi.json");

    write_openapi_spec(&unwritable);

    assert!(!unwritable.exists(), "the write should have failed");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── build_app / router smoke tests ───────────────────────────────────────────

#[tokio::test]
async fn put_machine_updates_name() {
    let dir = std::env::temp_dir().join(format!("moadim-machine-put-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(new_store(), crate::routines::new_store());
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
    let app = build_app(new_store(), crate::routines::new_store());
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
    let app = build_app(new_store(), crate::routines::new_store());
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
    let app = build_app(new_store(), crate::routines::new_store());
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
async fn build_app_serves_root() {
    let app = build_app(new_store(), crate::routines::new_store());
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
    let app = build_app(new_store(), crate::routines::new_store());
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
    let app = build_app(new_store(), crate::routines::new_store());
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
async fn build_app_sets_security_headers_on_ui_and_api() {
    // The whole router carries the security headers (issue #406): assert on a representative
    // UI response (the SPA at `/`) and a representative API response (`/api/v1/health`).
    for uri in ["/", "/api/v1/health"] {
        let resp = build_app(new_store(), crate::routines::new_store())
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
            "frame-ancestors 'none'"
        );
    }
}

#[tokio::test]
async fn build_app_serves_agents() {
    let app = build_app(new_store(), crate::routines::new_store());
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
    // Seed a cron job and a routine whose targeting lists overlap (`shared`) so the response
    // exercises de-duplication across both stores, plus the implicit local-identity entry.
    let store = new_store();
    store.lock().unwrap().insert(
        "j1".to_string(),
        crate::cron_jobs::CronJob {
            id: "j1".to_string(),
            schedule: "@daily".to_string(),
            handler: "h".to_string(),
            metadata: serde_json::Value::Null,
            machines: vec!["zeta-box".to_string(), "shared".to_string()],
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_manual_trigger_at: None,
        },
    );
    let routines = crate::routines::new_store();
    routines.lock().unwrap().insert(
        "r1".to_string(),
        crate::routines::Routine {
            id: "r1".to_string(),
            schedule: "@daily".to_string(),
            title: "R".to_string(),
            agent: "claude".to_string(),
            prompt: "p".to_string(),
            repositories: vec![],
            machines: vec!["alpha-box".to_string(), "shared".to_string()],
            tags: vec![],
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_manual_trigger_at: None,
            last_scheduled_trigger_at: None,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    let resp = build_app(store, routines)
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
        "zeta-box".to_string(),
    ];
    expected.sort();
    expected.dedup();
    assert_eq!(machines, expected);
}

#[tokio::test]
async fn build_app_serves_health() {
    let app = build_app(new_store(), crate::routines::new_store());
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
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    // The resolved machine name is surfaced so clients can identify which daemon answered.
    assert!(
        json["machine"].is_string() && !json["machine"].as_str().unwrap().is_empty(),
        "health payload should carry a non-empty machine name, got: {json}"
    );
}

#[tokio::test]
async fn build_app_serves_ui_at_root() {
    let app = build_app(new_store(), crate::routines::new_store());
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
    let app = build_app(new_store(), crate::routines::new_store());
    let resp = app
        .oneshot(Request::builder().uri("/ui").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resp.headers().get("location").unwrap(), "/");
}

#[tokio::test]
async fn build_app_spa_fallback_serves_ui_on_client_routes() {
    // `/cron-jobs` (and other client-routed paths) are NOT API endpoints — the API lives under
    // `/api/v1`. Unmatched GETs fall back to the app HTML so the Yew router can resolve the path.
    let app = build_app(new_store(), crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/cron-jobs")
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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

// ── cron-jobs CRUD lifecycle (covers all HTTP handlers + FromRef) ─────────────

#[tokio::test]
async fn router_cron_job_full_lifecycle() {
    let store = new_store();

    // POST /cron-jobs → 201
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"schedule":"@daily","handler":"test-h"}"#))
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

    // GET /cron-jobs → 200 (list)
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/cron-jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // GET /cron-jobs/{id} → 200
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // PATCH /cron-jobs/{id} → 200
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/cron-jobs/{id}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"handler":"patched"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // POST /cron-jobs/{id}/trigger → 200  (exercises FromRef<AppState> for CronStore)
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/cron-jobs/{id}/trigger"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // DELETE /cron-jobs/{id} → 200
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!crate::paths::job_dir(&id).exists());
}

#[tokio::test]
async fn router_handler_registered_true_when_handler_script_exists_on_disk() {
    // Regression test for the empty-registry bug: build the production AppState through the
    // real startup path (`build_app` -> `build_app_with_shutdown` -> `scan_registry`), not
    // `new_registry()` directly, so a registry that silently stays empty would be caught here.
    let _home = TempHome::set();
    let handlers_dir = crate::paths::handlers_dir();
    std::fs::create_dir_all(&handlers_dir).unwrap();
    std::fs::write(handlers_dir.join("real-handler.sh"), b"#!/bin/sh\n").unwrap();

    let store = new_store();
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schedule":"@daily","handler":"real-handler"}"#,
                ))
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
    assert_eq!(
        created["handler_registered"], true,
        "a job created against a handler script that already exists on disk should be \
         registered by the startup scan"
    );

    // A second request rebuilds the app (and re-scans the registry) the same way the real
    // server does on every request; the result should not depend on which request created it.
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let fetched: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(fetched["handler_registered"], true);
}

#[tokio::test]
async fn router_create_invalid_cron_returns_400() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"schedule":"bad","handler":"h"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn router_get_nonexistent_returns_404() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/cron-jobs/no-such-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_patch_nonexistent_returns_404() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/cron-jobs/no-such-id")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"handler":"h"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_delete_nonexistent_returns_404() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/cron-jobs/no-such-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_trigger_nonexistent_returns_404() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/cron-jobs/no-such-id/trigger")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_routines_cleanup_returns_removed_count() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines/cleanup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(val["removed"].is_u64());
}

// ── echo handler ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn echo_returns_message_and_timestamp() {
    let app = Router::new().route("/echo", post(echo));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"message":"hello"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["message"], "hello");
    assert!(json["timestamp"].as_u64().is_some());
}

#[tokio::test]
async fn echo_rejects_invalid_json() {
    let app = Router::new().route("/echo", post(echo));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from("not-json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn echo_rejects_missing_message_field() {
    let app = Router::new().route("/echo", post(echo));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"other":"field"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── logs endpoint ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn router_get_logs_nonexistent_returns_404() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/cron-jobs/no-such-id/logs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn router_get_logs_existing_returns_empty_when_no_file() {
    let store = new_store();
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"schedule":"@daily","handler":"log-h"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/cron-jobs/{id}/logs"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"");

    let _ = build_app(store, crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn router_get_logs_returns_file_content() {
    let store = new_store();
    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/cron-jobs")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"schedule":"@daily","handler":"log-h2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    let log_path = crate::paths::job_log_path(&id);
    tokio::fs::write(&log_path, "line1\nline2\n").await.unwrap();

    let resp = build_app(store.clone(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/cron-jobs/{id}/logs"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"line1\nline2\n");

    let _ = build_app(store, crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/cron-jobs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
}

// ── routines CRUD lifecycle (covers all routine HTTP handlers) ────────────────

#[tokio::test]
async fn router_routine_full_lifecycle() {
    let _home = TempHome::set();
    let store = new_store();
    let routines = crate::routines::new_store();

    let body = r#"{"schedule":"@daily","title":"Http Routine","agent":"claude","prompt":"p","repositories":[{"repository":"r","branch":"main"}]}"#;
    let resp = build_app(store.clone(), routines.clone())
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
    let resp = build_app(store.clone(), routines.clone())
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
    let resp = build_app(store.clone(), routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // PATCH
    let resp = build_app(store.clone(), routines.clone())
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
    let resp = build_app(store.clone(), routines.clone())
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
    let resp = build_app(store.clone(), routines.clone())
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
    let resp = build_app(store.clone(), routines.clone())
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
    let resp = build_app(store.clone(), routines.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/routines/{id}/logs"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // DELETE
    let resp = build_app(store.clone(), routines.clone())
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
async fn router_routine_create_invalid_cron_400() {
    let resp = build_app(new_store(), crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schedule":"bad","title":"t","agent":"a","prompt":"p"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn router_routine_not_found_paths() {
    for (method, suffix) in [
        ("GET", ""),
        ("DELETE", ""),
        ("POST", "/trigger"),
        ("POST", "/scheduled-trigger"),
        ("GET", "/logs"),
    ] {
        let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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

// ── run_with_listener integration test (real TCP) ────────────────────────────

#[tokio::test]
async fn run_with_listener_serves_over_tcp() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let store = new_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = tokio::spawn(run_with_listener_until(
        store,
        crate::routines::new_store(),
        listener,
        std::future::pending(),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut buf = vec![0u8; 512];
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");

    handle.abort();
}

#[tokio::test]
async fn build_app_shutdown_route_acknowledges() {
    let app = build_app(new_store(), crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/shutdown")
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
    assert_eq!(json["status"], "shutting down");
}

#[tokio::test]
async fn build_app_restart_route_acknowledges() {
    // The route spawns a detached `current_exe --background` helper; under the test harness that exe
    // is the test binary, which rejects `--background` and exits at once, so no real server starts.
    // TempHome keeps the helper's log file out of the real home.
    let _home = TempHome::set();
    let app = build_app(new_store(), crate::routines::new_store());
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
async fn shutdown_route_stops_the_serving_loop() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(run_with_listener_until(
        new_store(),
        crate::routines::new_store(),
        listener,
        std::future::pending(),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Hit /shutdown; the serving future should then resolve on its own.
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    stream
        .write_all(
            b"POST /api/v1/shutdown HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut buf = vec![0u8; 512];
    let n = stream.read(&mut buf).await.unwrap();
    assert!(
        String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 200"),
        "shutdown should be acknowledged"
    );

    // The server task must finish without being aborted.
    let joined = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
    assert!(joined.is_ok(), "server did not shut down after /shutdown");
    assert!(joined.unwrap().unwrap().is_ok());
}

#[tokio::test]
async fn run_with_listener_until_exits_on_immediate_shutdown() {
    let store = new_store();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let result =
        run_with_listener_until(store, crate::routines::new_store(), listener, async {}).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn mcp_endpoint_triggers_factory() {
    let app = build_app(new_store(), crate::routines::new_store());
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header(CONTENT_TYPE, "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("host", "localhost")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().as_u16() < 500);
}

#[tokio::test]
async fn router_serves_routines_ical_feed() {
    let routines = crate::routines::new_store();
    routines.lock().unwrap().insert(
        "r1".to_string(),
        crate::routines::Routine {
            id: "r1".to_string(),
            schedule: "@daily".to_string(),
            title: "My Routine".to_string(),
            agent: "claude".to_string(),
            prompt: "do the thing".to_string(),
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_manual_trigger_at: None,
            last_scheduled_trigger_at: None,
            tags: vec![],
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    let resp = build_app(new_store(), routines)
        .oneshot(
            Request::builder()
                .uri("/api/v1/routines.ics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_TYPE).unwrap(),
        "text/calendar; charset=utf-8"
    );
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.starts_with("BEGIN:VCALENDAR"));
    assert!(body.contains("BEGIN:VEVENT"));
    assert!(body.contains("SUMMARY:My Routine"));
}

// ── Global lock endpoints ─────────────────────────────────────────────────────

#[tokio::test]
async fn get_lock_status_returns_unlocked_by_default() {
    let _home = TempHome::set();
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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
    let resp = build_app(new_store(), crate::routines::new_store())
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

#[tokio::test]
async fn router_serves_per_routine_ical_feed_via_query() {
    // `GET /routines.ics?routine=<id>` scopes the feed to one routine and names the
    // calendar after it; an unknown id returns a well-formed empty calendar (issue #263).
    let routines = crate::routines::new_store();
    let mk = |id: &str, title: &str| crate::routines::Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        repositories: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        machines: vec![],
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    };
    {
        let mut lock = routines.lock().unwrap();
        lock.insert("a".to_string(), mk("a", "Routine A"));
        lock.insert("b".to_string(), mk("b", "Routine B"));
    }

    let fetch = |uri: &'static str| {
        let app = build_app(new_store(), routines.clone());
        async move {
            let resp = app
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        }
    };

    let filtered = fetch("/api/v1/routines.ics?routine=a").await;
    assert!(filtered.contains("UID:a-"));
    assert!(!filtered.contains("UID:b-"));
    assert!(filtered.contains("X-WR-CALNAME:Routine A\r\n"));

    let unknown = fetch("/api/v1/routines.ics?routine=missing").await;
    assert!(unknown.starts_with("BEGIN:VCALENDAR"));
    assert!(unknown.ends_with("END:VCALENDAR\r\n"));
    assert_eq!(unknown.matches("BEGIN:VEVENT").count(), 0);
}

#[tokio::test]
async fn health_uptime_clamps_to_zero_on_backward_clock_skew() {
    // A `uptime_start` in the future models the wall clock jumping backward
    // after the server started. The old `now_secs() - uptime_start` would
    // underflow; saturating_sub must clamp uptime to 0 instead.
    let state = AppState {
        store: new_store(),
        handlers: new_registry(),
        routines: crate::routines::new_store(),
        uptime_start: now_secs() + 10_000,
        shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
    };
    let resp = health(axum::extract::State(state)).await;
    assert_eq!(resp.0.uptime_secs, 0);
    assert_eq!(resp.0.status, "ok");
    assert!(resp.0.running);
}

#[tokio::test]
async fn build_app_restart_route_returns_500_when_spawn_fails() {
    // Cover the `map_err(|_| AppError::Internal)?` branch in the restart handler (http.rs L139):
    // make spawn_restart() fail by placing a regular file at the `.config` component of the
    // home path so create_dir_all() for the daemon log directory errors out.
    let dir = std::env::temp_dir().join(format!("moadim-restart-fail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    // A regular file at `.config` blocks create_dir_all(".config/moadim") inside spawn_detached_with.
    std::fs::write(dir.join(".config"), b"blocker").unwrap();
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }

    let app = build_app(new_store(), crate::routines::new_store());
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
