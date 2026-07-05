#![allow(clippy::missing_docs_in_private_items)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
    routing::get,
    Router,
};
use tower::ServiceExt;
use tower_http::catch_panic::CatchPanicLayer;

use super::{build_app, health, run_with_listener_until, AppState};
use crate::utils::time::now_secs;

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

// ── run_with_listener integration test (real TCP) ────────────────────────────

#[tokio::test]
async fn run_with_listener_serves_over_tcp() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = tokio::spawn(run_with_listener_until(
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
    let app = build_app(crate::routines::new_store());
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
async fn shutdown_route_stops_the_serving_loop() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(run_with_listener_until(
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let result = run_with_listener_until(crate::routines::new_store(), listener, async {}).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn mcp_endpoint_triggers_factory() {
    let app = build_app(crate::routines::new_store());
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
            model: None,
            id: "r1".to_string(),
            schedule: "@daily".to_string(),
            title: "My Routine".to_string(),
            agent: "claude".to_string(),
            prompt: "do the thing".to_string(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            source: "managed".to_string(),
            created_at: 0,
            updated_at: 0,
            last_manual_trigger_at: None,
            last_scheduled_trigger_at: None,
            snoozed_until: None,
            skip_runs: None,
            tags: vec![],
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    let resp = build_app(routines)
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
        model: None,
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
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
        let app = build_app(routines.clone());
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

/// `CatchPanicLayer` is what stands between a panicking handler and a reset connection with no
/// response (issue #337). `build_app`'s production routes never panic deliberately, so exercise
/// the layer directly on a minimal router wired the same way, confirming it turns a panic into a
/// plain 500 instead of the request erroring out.
#[tokio::test]
async fn catch_panic_layer_turns_a_handler_panic_into_a_500() {
    async fn boom() -> StatusCode {
        panic!("intentional test panic")
    }

    let app = Router::new()
        .route("/boom", get(boom))
        .layer(CatchPanicLayer::new());

    let resp = app
        .oneshot(Request::builder().uri("/boom").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
