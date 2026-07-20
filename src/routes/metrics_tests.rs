#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt;

use super::{render, MetricsSnapshot, PROMETHEUS_CONTENT_TYPE};
use crate::routes::http::build_app;
use crate::routines::{FleetRunSummary, RunStatus};

/// A minimal [`FleetRunSummary`] fixture; `started_at`/`finished_at`/`status` are the only
/// fields [`render`] reads, but every field is required to construct one.
fn run(started_at: u64, finished_at: Option<u64>, status: RunStatus) -> FleetRunSummary {
    FleetRunSummary {
        routine_id: "routine-1".to_string(),
        routine_title: "Test routine".to_string(),
        workbench: format!("test-routine-{started_at}"),
        started_at,
        started_at_local: String::new(),
        finished_at,
        finished_at_local: None,
        status,
        exit_code: match status {
            RunStatus::Success => Some(0),
            RunStatus::Failed => Some(1),
            RunStatus::Running | RunStatus::Unknown => None,
        },
    }
}

fn empty_snapshot(runs: &[FleetRunSummary]) -> MetricsSnapshot<'_> {
    MetricsSnapshot {
        uptime_secs: 42,
        version: "1.2.3",
        git_sha: "abc123",
        machine: "test-machine",
        active_sessions: 2,
        workbench_bytes: 1024,
        runs,
        cleanup_removed_total: 5,
        cleanup_freed_bytes_total: 2048,
    }
}

#[test]
fn render_emits_help_and_type_for_every_gauge_and_counter() {
    let text = render(&empty_snapshot(&[]));
    for name in [
        "moadim_uptime_seconds",
        "moadim_build_info",
        "moadim_active_sessions",
        "moadim_workbench_bytes",
        "moadim_runs_total",
        "moadim_run_duration_seconds",
        "moadim_cleanup_removed_total",
        "moadim_cleanup_freed_bytes_total",
    ] {
        assert!(
            text.contains(&format!("# HELP {name} ")),
            "missing HELP line for {name} in:\n{text}"
        );
        assert!(
            text.contains(&format!("# TYPE {name} ")),
            "missing TYPE line for {name} in:\n{text}"
        );
    }
    assert!(text.contains("moadim_uptime_seconds 42"));
    assert!(text.contains(
        r#"moadim_build_info{version="1.2.3",git_sha="abc123",machine="test-machine"} 1"#
    ));
    assert!(text.contains("moadim_active_sessions 2"));
    assert!(text.contains("moadim_workbench_bytes 1024"));
    assert!(text.contains("moadim_cleanup_removed_total 5"));
    assert!(text.contains("moadim_cleanup_freed_bytes_total 2048"));
}

#[test]
fn render_escapes_a_machine_name_containing_prometheus_label_syntax() {
    let mut snapshot = empty_snapshot(&[]);
    snapshot.machine = "weird\"machine\\name\nwith-newline";
    let text = render(&snapshot);
    assert!(
        text.contains(
            r#"moadim_build_info{version="1.2.3",git_sha="abc123",machine="weird\"machine\\name\nwith-newline"} 1"#
        ),
        "machine label was not escaped as expected in:\n{text}"
    );
    // The raw quote/backslash must not appear unescaped anywhere in the output — an unescaped
    // one would break every other metric on the same scrape, not just this line.
    assert!(!text.contains(r#"machine="weird"machine"#));
}

#[test]
fn render_runs_total_counts_each_status_independently() {
    let runs = [
        run(100, Some(110), RunStatus::Success),
        run(200, Some(210), RunStatus::Success),
        run(300, Some(310), RunStatus::Failed),
        run(400, None, RunStatus::Running),
        run(500, Some(510), RunStatus::Unknown),
    ];
    let text = render(&empty_snapshot(&runs));
    assert!(text.contains(r#"moadim_runs_total{status="success"} 2"#));
    assert!(text.contains(r#"moadim_runs_total{status="failed"} 1"#));
    assert!(text.contains(r#"moadim_runs_total{status="running"} 1"#));
    assert!(text.contains(r#"moadim_runs_total{status="unknown"} 1"#));
}

#[test]
fn render_run_duration_histogram_buckets_cumulatively_by_le() {
    // Durations of 3s and 100s land in different buckets; the still-running run (no
    // `finished_at`) must be excluded from the histogram entirely.
    let runs = [
        run(0, Some(3), RunStatus::Success),
        run(0, Some(100), RunStatus::Failed),
        run(0, None, RunStatus::Running),
    ];
    let text = render(&empty_snapshot(&runs));
    // The 3s run is <= every finite bucket bound (5, 15, 30, ...).
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="5"} 1"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="15"} 1"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="30"} 1"#));
    // The 100s run joins once the bound reaches 120.
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="60"} 1"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="120"} 2"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="300"} 2"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="600"} 2"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="1800"} 2"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="3600"} 2"#));
    assert!(text.contains(r#"moadim_run_duration_seconds_bucket{le="+Inf"} 2"#));
    assert!(text.contains("moadim_run_duration_seconds_sum 103"));
    assert!(text.contains("moadim_run_duration_seconds_count 2"));
}

#[tokio::test]
async fn build_app_serves_metrics_with_prometheus_content_type() {
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_TYPE).unwrap(),
        PROMETHEUS_CONTENT_TYPE,
    );
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.starts_with("# HELP moadim_uptime_seconds"));
    // A fresh in-memory store with no routines still reports every series (all zero), not an
    // error or an empty body.
    assert!(text.contains("moadim_runs_total{status=\"success\"} 0"));
    assert!(text.contains("moadim_run_duration_seconds_count 0"));
}
