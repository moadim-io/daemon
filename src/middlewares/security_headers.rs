use axum::{
    extract::Request,
    http::{header, header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Defense-in-depth response headers applied to every HTTP response (UI + `/api/v1`).
///
/// The daemon serves a browser dashboard whose buttons drive an unauthenticated loopback API
/// (create / trigger / delete routines, `POST /shutdown`), so the served responses are hardened
/// against the cheap framing / sniffing / in-page-injection vectors:
///
/// - `X-Frame-Options: DENY` + CSP `frame-ancestors 'none'` — block clickjacking of the
///   dashboard's destructive controls via `<iframe src="http://localhost:5784/">`.
/// - `X-Content-Type-Options: nosniff` — stop browsers content-sniffing a response into an
///   unintended type.
/// - `Referrer-Policy: no-referrer` — never leak the loopback URL to third parties.
/// - A CSP that locks everything to `'self'` by default and blocks `object-src`, `base-uri`, and
///   `form-action` outright — so an injected `<script>`, `<base>`, or off-origin `<form>` cannot
///   exfiltrate to or act on behalf of the unauthenticated destructive API (issue #406's
///   in-code follow-up, #551).
///
/// Two directives stay loose, by necessity rather than oversight:
///
/// - `script-src` / `style-src` carry `'unsafe-inline'`. The bundled Yew/WASM SPA
///   (`prebuilt.html`, built by `src/build/ui.rs`) self-inlines its entire wasm-bindgen JS glue
///   and base64 WASM payload into a single `<script type="module">` so the daemon ships as one
///   file with no separate static-asset serving; that payload's bytes change on every UI rebuild,
///   so a `sha256-…` hash would have to be regenerated and wired through the build script rather
///   than hardcoded, which is left as further follow-up. The page also has one inline `<style>`
///   block, and the embedded Swagger UI (`utoipa-swagger-ui`) sets inline `style="…"` attributes
///   from its React components at runtime. `script-src` additionally carries
///   `'wasm-unsafe-eval'`, required for the SPA's `WebAssembly.instantiate` call — narrower than
///   `'unsafe-eval'`, it permits WASM compilation without permitting `eval()`/`Function()`. The
///   React `client/` bundle served at `/client` (`prebuilt-client.html`, built by
///   `src/build/client.rs`) is inlined the same way by `vite-plugin-singlefile` — one inline
///   `<script type="module">` and one inline `<style>` — so it needs the same `'unsafe-inline'`
///   allowance, but has no WASM and so never needs `'wasm-unsafe-eval'` itself. This is one
///   blanket policy applied to every response, `ui/` and `client/` alike; it can't be tightened
///   per-route without splitting the middleware, which isn't warranted while both SPAs are served.
/// - `style-src` and `font-src` allow `https://fonts.googleapis.com` / `https://fonts.gstatic.com`
///   respectively: the dashboard still loads its webfont from Google Fonts pending #467 (tracked
///   separately in open PR #519); once that self-hosts the font, these CDN allowances should be
///   dropped in favor of `'self'`.
///
/// `img-src` allows `data:` for an inline SVG noise-texture background used by the dashboard CSS.
const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("x-frame-options", "DENY"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    (
        "content-security-policy",
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
         style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
         font-src 'self' https://fonts.gstatic.com; \
         img-src 'self' data:; \
         connect-src 'self'; \
         base-uri 'none'; \
         form-action 'none'; \
         object-src 'none'; \
         frame-ancestors 'none'",
    ),
];

/// Inject the [`SECURITY_HEADERS`] onto every response.
///
/// `Cache-Control: no-store` is also added as a fallback for responses that don't already carry a
/// `Cache-Control` header. Responses that set their own directive (e.g. the SPA's `index.html`
/// uses `no-cache` to enable ETag-based 304 revalidation, issue #401) are left untouched.
/// Without this fallback, browsers apply heuristic caching to API `GET` responses and may serve
/// stale JSON on page reload (issue #921).
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
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    res
}

#[cfg(test)]
#[path = "security_headers_tests.rs"]
mod security_headers_tests;
