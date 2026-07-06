/// Middleware that rejects browser-borne cross-origin requests (DNS rebinding, CSRF-style abuse).
pub mod host_validation;
/// Middleware that logs request method, path, status, and latency.
pub mod logger;
/// Middleware that adds defense-in-depth security response headers.
pub mod security_headers;
/// Middleware that bounds REST API handler time with a per-request deadline.
pub mod timeout;
