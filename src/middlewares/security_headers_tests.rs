#![allow(clippy::missing_docs_in_private_items)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use tower::ServiceExt;

use super::{security_headers, SECURITY_HEADERS};

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(security_headers))
}

#[tokio::test]
async fn security_headers_present_on_response() {
    let resp = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    for (name, value) in SECURITY_HEADERS {
        assert_eq!(
            resp.headers().get(*name).map(|hv| hv.to_str().unwrap()),
            Some(*value),
            "missing or wrong {name} header"
        );
    }
}

#[tokio::test]
async fn frame_ancestors_blocks_framing() {
    let resp = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
    // Full CSP header value (#551): `frame-ancestors 'none'` is retained from #406, plus the
    // hardening described on `SECURITY_HEADERS` — `default-src 'self'` with explicit `object-src`
    // / `base-uri` / `form-action` denials so an injected `<script>`, `<base>`, or off-origin
    // `<form>` cannot act on behalf of the unauthenticated destructive API.
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
