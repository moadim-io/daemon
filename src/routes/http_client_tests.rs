//! Tests for `/client` (the React client, served alongside `ui/` — see `CLIENT_HTML`'s doc
//! comment in `http.rs`). Split out of `http_tests.rs` to keep that file under the line-count
//! gate, mirroring the existing `http_settings_routes_tests.rs` split.

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt;

use super::build_app;

#[tokio::test]
async fn build_app_serves_client() {
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
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("text/html"));
}

#[tokio::test]
async fn build_app_serves_client_with_etag() {
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
async fn build_app_returns_304_for_client_when_if_none_match_matches() {
    let app = build_app(crate::routines::new_store());
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/client")
                .body(Body::empty())
                .unwrap(),
        )
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
                .uri("/client")
                .header(axum::http::header::IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(body.is_empty(), "304 response must not carry a body");
}

#[tokio::test]
async fn build_app_client_fallback_serves_client_on_nested_routes() {
    // `/client/routines` (and other React-Router-owned paths) aren't real server routes — the
    // nested `/client` router's own fallback returns the same HTML so React Router resolves the
    // path client-side on a hard refresh or deep link, mirroring `/routines` for `ui/` at `/`.
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
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("text/html"));
}

#[tokio::test]
async fn build_app_client_and_root_serve_distinct_bundles() {
    // `/client` and `/` embed two independent SPAs (`CLIENT_HTML` vs `INDEX_HTML`) — confirm the
    // nested router isn't accidentally aliased to the outer `index` handler.
    let app = build_app(crate::routines::new_store());
    let root = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let client = app
        .oneshot(
            Request::builder()
                .uri("/client")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let root_etag = root
        .headers()
        .get(axum::http::header::ETAG)
        .unwrap()
        .clone();
    let client_etag = client
        .headers()
        .get(axum::http::header::ETAG)
        .unwrap()
        .clone();
    assert_ne!(root_etag, client_etag);
}

/// Every unmatched path outside `/client` (and `/api/v1`) must keep falling through to `ui/`'s
/// `INDEX_HTML`, not the React client — the outer `.fallback(get(index))` is untouched by the
/// new `/client` nest.
#[tokio::test]
async fn build_app_root_fallback_unaffected_by_client_route() {
    let app = build_app(crate::routines::new_store());
    let root = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let root_etag = root
        .headers()
        .get(axum::http::header::ETAG)
        .unwrap()
        .clone();

    let unmatched = app
        .oneshot(
            Request::builder()
                .uri("/some/unmatched/path")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unmatched.status(), StatusCode::OK);
    assert_eq!(
        unmatched.headers().get(axum::http::header::ETAG).unwrap(),
        &root_etag
    );
}
