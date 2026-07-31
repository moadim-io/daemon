
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
    // The iCal feed reloads from disk first, so the routine must be persisted to the (temp-home)
    // routines dir; an in-memory-only insert would be wiped by the reload.
    let _home = TempHome::set();
    crate::routine_storage::write_routine(&crate::routines::Routine {
        model: None,
        id: "r1".to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
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
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    })
    .unwrap();
    let resp = build_app(crate::routines::new_store())
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

/// `CatchPanicLayer` is what stands between a panicking handler and a reset connection with no
/// response (issue #337). `build_app`'s production routes never panic deliberately, so exercise
/// the layer directly on a minimal router wired the same way, confirming it turns a panic into a
/// plain 500 instead of the request erroring out.
#[cfg(test)]
#[path = "http_listener_lock_tests.rs"]
mod http_listener_lock_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "router_serves_per_routine_ical_feed_via_query_tests.rs"]
mod router_serves_per_routine_ical_feed_via_query_tests;
