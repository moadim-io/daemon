#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{body::Body, http::Request};
use tower::ServiceExt;

use crate::routes::http::build_app;
use crate::sync::{record_crontab_sync_failure, reset_crontab_sync_status_for_tests, SyncError};

struct CronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl CronShim {
    fn succeeding() -> Self {
        Self::with_script("#!/bin/sh\nif [ \"$1\" = \"-l\" ]; then exit 0; fi\ncat >/dev/null\n")
    }

    fn failing() -> Self {
        Self::with_script("#!/bin/sh\necho shim failed >&2\nexit 1\n")
    }

    fn with_script(content: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-retry-cron-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let script = base.join("crontab");
        std::fs::write(&script, content).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: these route tests are run single-threaded by the repo's test harness.
        unsafe { std::env::set_var("MOADIM_CRONTAB_BIN", &script) };
        Self { base, previous }
    }
}

impl Drop for CronShim {
    fn drop(&mut self) {
        // SAFETY: these route tests are run single-threaded by the repo's test harness.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[tokio::test]
async fn retry_endpoint_returns_healthy_status_after_successful_sync() {
    let _shim = CronShim::succeeding();
    record_crontab_sync_failure(&SyncError::CrontabCommand("old failure".into()));
    let app = build_app(crate::routines::new_store());

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/crontab-sync/retry")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.status().is_success());
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["crontab_sync"]["ok"], true);
    assert_eq!(body["crontab_sync"]["last_error"], serde_json::Value::Null);
    reset_crontab_sync_status_for_tests();
}

#[tokio::test]
async fn retry_endpoint_reports_failed_sync_in_health_body() {
    let _shim = CronShim::failing();
    reset_crontab_sync_status_for_tests();
    let app = build_app(crate::routines::new_store());

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/crontab-sync/retry")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.status().is_success());
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["crontab_sync"]["ok"], false);
    assert!(body["crontab_sync"]["last_error"]
        .as_str()
        .unwrap()
        .contains("crontab"));
    assert!(body["crontab_sync"]["last_error_at"].is_number());
    reset_crontab_sync_status_for_tests();
}
