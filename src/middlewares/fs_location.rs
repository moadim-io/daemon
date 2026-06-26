use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

/// Inject server filesystem location into response headers.
pub async fn fs_location(req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    let loc = crate::filesystem::FsLocation::current();
    let val = serde_json::to_value(&loc).unwrap_or_default();
    inject_headers_from_value(res, val)
}

/// Inject fields from a JSON object value as `x-*` response headers.
fn inject_headers_from_value(mut res: Response, val: serde_json::Value) -> Response {
    let serde_json::Value::Object(map) = val else {
        return res;
    };
    for (key, value) in map {
        let serde_json::Value::String(str_value) = value else {
            continue;
        };
        let name = format!("x-{}", key.replace('_', "-"));
        if let (Ok(header_name), Ok(header_value)) = (
            axum::http::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&str_value),
        ) {
            res.headers_mut().insert(header_name, header_value);
        }
    }
    res
}

#[cfg(test)]
#[path = "fs_location_tests.rs"]
mod fs_location_tests;
