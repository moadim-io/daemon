use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;

/// The health-check endpoint the web UI polls continuously. Logged at `debug`
/// (see [`logger`]). This is the fully-qualified request path the middleware
/// sees, since the logger layers over the outer router after the `/api/v1` nest.
const HEALTH_PATH: &str = "/api/v1/health";

/// Log method, path, status, and latency for each request.
///
/// `GET /api/v1/health` is logged at `debug` rather than `info`: the web UI
/// polls it continuously, so at the default `info` level it would otherwise
/// dominate `daemon.log` (two lines per poll, thousands of lines a day on an
/// idle UI) and bury every other request. It stays available under
/// `RUST_LOG=debug`.
pub async fn logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    // Health-check polls are noise at info level; keep them at debug.
    let level = if path == HEALTH_PATH {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    log::log!(level, "{} {}", method, path);
    let start = Instant::now();
    let res = next.run(req).await;
    log::log!(
        level,
        "  -> {} {} in {}ms",
        res.status(),
        path,
        start.elapsed().as_millis()
    );
    res
}

#[cfg(test)]
#[path = "logger_tests.rs"]
mod logger_tests;
