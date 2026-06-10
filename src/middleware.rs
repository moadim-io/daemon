//! Axum middleware layers shared across routes.

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
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

pub async fn fs_location(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let loc = crate::fs_location::FsLocation::current();
    if let Ok(serde_json::Value::Object(map)) = serde_json::to_value(&loc) {
        for (k, v) in map {
            if let serde_json::Value::String(s) = v {
                let name = format!("x-{}", k.replace('_', "-"));
                if let (Ok(n), Ok(v)) = (
                    axum::http::HeaderName::from_bytes(name.as_bytes()),
                    HeaderValue::from_str(&s),
                ) {
                    res.headers_mut().insert(n, v);
                }
            }
        }
    }
    res
}
