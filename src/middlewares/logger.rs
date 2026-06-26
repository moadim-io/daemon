use axum::{extract::Request, middleware::Next, response::Response};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Monotonic source of per-request correlation ids. Wrapping on overflow is
/// harmless: an id only needs to be unique among the requests currently
/// in-flight so a response line can be matched to its request line.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Log method, path, status, and latency for each request.
///
/// Each request is tagged with a short correlation id that prefixes both its
/// inbound (`<-`) and outbound (`->`) line, so the two can be paired in the log
/// even when many requests interleave under concurrency.
pub async fn logger(req: Request, next: Next) -> Response {
    let id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    log::info!("[{id:08x}] <- {method} {path}");
    let start = Instant::now();
    let res = next.run(req).await;
    log::info!(
        "[{id:08x}] -> {} {} in {}ms",
        res.status(),
        path,
        start.elapsed().as_millis()
    );
    res
}

#[cfg(test)]
#[path = "logger_tests.rs"]
mod logger_tests;
