use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

/// Inject server filesystem location into response headers.
pub async fn fs_location(req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    let loc = crate::fs_location::FsLocation::current();
    let val = serde_json::to_value(&loc).unwrap_or_default();
    inject_headers_from_value(res, val)
}

/// Inject fields from a JSON object value as `x-*` response headers.
fn inject_headers_from_value(mut res: Response, val: serde_json::Value) -> Response {
    let map = match val {
        serde_json::Value::Object(m) => m,
        _ => return res,
    };
    for (k, v) in map {
        let s = match v {
            serde_json::Value::String(s) => s,
            _ => continue,
        };
        let name = format!("x-{}", k.replace('_', "-"));
        if let (Ok(n), Ok(v)) = (
            axum::http::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&s),
        ) {
            res.headers_mut().insert(n, v);
        }
    }
    res
}

#[cfg(test)]
#[path = "fs_location_tests.rs"]
mod fs_location_tests;
