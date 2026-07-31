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

#[test]
fn write_openapi_spec_skips_rewrite_when_unchanged() {
    // A second call with identical content must not rewrite the file, so dev startups don't churn
    // the committed spec's mtime on every run.
    let dir = std::env::temp_dir().join(format!("moadim-openapi-nochurn-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("openapi.json");

    write_openapi_spec(&path);
    let first = std::fs::metadata(&path).unwrap().modified().unwrap();
    write_openapi_spec(&path);
    let second = std::fs::metadata(&path).unwrap().modified().unwrap();

    assert_eq!(first, second, "unchanged spec should not be rewritten");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── build_app / router smoke tests ───────────────────────────────────────────

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
include!("http_tests_part3.rs");
