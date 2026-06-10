use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

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
