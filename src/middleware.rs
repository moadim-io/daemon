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
    if let Some(root) = loc.server_root {
        if let Ok(val) = HeaderValue::from_str(&root) {
            res.headers_mut().insert("x-server-root", val);
        }
    }
    if let Some(exe_dir) = loc.server_exe_dir {
        if let Ok(val) = HeaderValue::from_str(&exe_dir) {
            res.headers_mut().insert("x-server-exe-dir", val);
        }
    }
    res
}
