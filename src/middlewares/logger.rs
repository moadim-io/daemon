use axum::{extract::Request, middleware::Next, response::Response};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// The health-check endpoint the web UI polls continuously. Logged at `debug`
/// (see [`logger`]). This is the fully-qualified request path the middleware
/// sees, since the logger layers over the outer router after the `/api/v1` nest.
const HEALTH_PATH: &str = "/api/v1/health";

/// Monotonic source of per-request correlation ids. Wrapping on overflow is
/// harmless: an id only needs to be unique among the requests currently
/// in-flight so a response line can be matched to its request line.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Log method, path, status, and latency for each request.
///
/// `GET /api/v1/health` is logged at `debug` rather than `info`: the web UI
/// polls it continuously, so at the default `info` level it would otherwise
/// dominate `daemon.log` (two lines per poll, thousands of lines a day on an
/// idle UI) and bury every other request. It stays available under
/// `RUST_LOG=debug`.
///
/// Each request is tagged with a short correlation id that prefixes both its
/// inbound (`<-`) and outbound (`->`) line, so the two can be paired in the log
/// even when many requests interleave under concurrency.
pub async fn logger(req: Request, next: Next) -> Response {
    let id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    // Health-check polls are noise at info level; keep them at debug.
    let level = if path == HEALTH_PATH {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    log::log!(level, "[{id:08x}] <- {method} {path}");
    let start = Instant::now();
    let res = next.run(req).await;
    let status = res.status();
    let elapsed = start.elapsed().as_millis();
    log::log!(level, "[{id:08x}] -> {status} {path} in {elapsed}ms");
    res
}

#[cfg(test)]
#[path = "logger_tests.rs"]
mod logger_tests;
