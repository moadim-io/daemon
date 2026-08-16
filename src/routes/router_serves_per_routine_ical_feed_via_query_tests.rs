#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[tokio::test]
async fn router_serves_per_routine_ical_feed_via_query() {
    // `GET /routines.ics?routine=<id>` scopes the feed to one routine and names the
    // calendar after it; an unknown id returns a well-formed empty calendar (issue #263).
    // The iCal feed reloads from disk first, so the routines must be persisted to the (temp-home)
    // routines dir; in-memory-only inserts would be wiped by the reload.
    let _home = TempHome::set();
    let mk = |id: &str, title: &str| crate::routines::Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        model: None,
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        machines: vec![],
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    };
    crate::routine_storage::write_routine(&mk("a", "Routine A")).unwrap();
    crate::routine_storage::write_routine(&mk("b", "Routine B")).unwrap();

    let fetch = |uri: &'static str| {
        let app = build_app(crate::routines::new_store());
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

#[tokio::test]
async fn serve_with_grace_returns_serve_result_when_serve_finishes_first() {
    // No shutdown is ever requested (`pending`); the server returns on its own and its result
    // propagates unchanged.
    let out = serve_with_grace(
        async { Ok(()) },
        std::future::pending::<()>(),
        Duration::from_secs(60),
    )
    .await;
    assert!(out.is_ok());
}

#[tokio::test]
async fn serve_with_grace_propagates_serve_error_before_shutdown() {
    let out = serve_with_grace(
        async { Err(std::io::Error::other("serve failed")) },
        std::future::pending::<()>(),
        Duration::from_secs(60),
    )
    .await;
    assert!(out.is_err(), "a serve error before shutdown must surface");
}

#[tokio::test]
async fn serve_with_grace_drains_within_grace_after_shutdown() {
    // Shutdown fires immediately, then the server drains well inside the grace window: its own
    // result is returned (no forced exit).
    let serve = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    };
    let out = serve_with_grace(serve, async {}, Duration::from_secs(60)).await;
    assert!(out.is_ok());
}

#[tokio::test]
async fn serve_with_grace_forces_exit_when_connections_never_close() {
    // The server never returns (modeling an open `/mcp` SSE stream pinning the connection). After
    // the grace window the wrapper forces a clean exit instead of hanging forever (#342).
    let start = std::time::Instant::now();
    let out = serve_with_grace(
        std::future::pending::<std::io::Result<()>>(),
        async {},
        Duration::from_millis(20),
    )
    .await;
    assert!(out.is_ok(), "a forced exit reports success");
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "must force exit at the grace deadline, not hang"
    );
}

#[test]
fn shutdown_grace_honors_env_override_then_falls_back() {
    // SAFETY: tests in this crate run single-threaded per binary, so env mutation is race-free.
    unsafe {
        std::env::set_var(SHUTDOWN_GRACE_MS_ENV, "42");
    }
    assert_eq!(shutdown_grace(), Duration::from_millis(42));
    // An unparseable value falls back to the compiled default.
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var(SHUTDOWN_GRACE_MS_ENV, "not-a-number");
    }
    assert_eq!(shutdown_grace(), SHUTDOWN_GRACE);
    // An unset value also falls back.
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::remove_var(SHUTDOWN_GRACE_MS_ENV);
    }
    assert_eq!(shutdown_grace(), SHUTDOWN_GRACE);
}
