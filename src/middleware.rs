//! Axum middleware layers shared across routes.

use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;

/// Log each request method, path, response status, and elapsed time.
pub async fn logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    log::info!("{} {}", method, path);
    let start = Instant::now();
    let res = next.run(req).await;
    log::info!(
        "  -> {} {} in {}ms",
        res.status(),
        path,
        start.elapsed().as_millis()
    );
    res
}
