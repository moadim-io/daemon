use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Defense-in-depth response headers applied to every HTTP response (UI + `/api/v1`).
///
/// The daemon serves a browser dashboard whose buttons drive an unauthenticated loopback API
/// (create / trigger / delete routines, `POST /shutdown`), so the served responses are hardened
/// against the cheap framing / sniffing vectors:
///
/// - `X-Frame-Options: DENY` + CSP `frame-ancestors 'none'` — block clickjacking of the
///   dashboard's destructive controls via `<iframe src="http://localhost:5784/">`.
/// - `X-Content-Type-Options: nosniff` — stop browsers content-sniffing a response into an
///   unintended type.
/// - `Referrer-Policy: no-referrer` — never leak the loopback URL to third parties.
///
/// The CSP is intentionally scoped to `frame-ancestors` only: it closes the framing gap without
/// constraining `script-src` / `style-src`, so the existing inline + WASM SPA and the Swagger UI
/// keep working untouched. A fuller `script-src`/`style-src` policy is left as follow-up once it
/// can be verified against the bundled UI.
const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("x-frame-options", "DENY"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("content-security-policy", "frame-ancestors 'none'"),
];

/// Inject the [`SECURITY_HEADERS`] onto every response.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        // Names and values are static, lowercase, printable ASCII, so `from_static` cannot panic.
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    res
}

#[cfg(test)]
#[path = "security_headers_tests.rs"]
mod security_headers_tests;
