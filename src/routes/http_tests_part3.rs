
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
             script-src 'self' 'unsafe-inline'; \
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

#[cfg(test)]
#[path = "http_settings_routes_tests.rs"]
mod http_settings_routes_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "http_tests_part2.rs"]
mod http_tests_part2;
