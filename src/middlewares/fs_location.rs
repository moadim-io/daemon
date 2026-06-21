use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

/// Opt-in env var that re-enables the `x-server-*` filesystem-location headers.
///
/// The headers expose absolute server paths (the CWD and the executable's dir,
/// hence the OS username and install layout) on *every* response, including
/// before any auth. That is needless information disclosure with no functional
/// consumer — the same data is available to intentional callers via
/// `GET /api/v1/health` and the MCP `health` tool — so the middleware stays off
/// unless an operator explicitly opts in for debugging by setting this to a
/// truthy value.
const DEBUG_FS_HEADERS_ENV: &str = "MOADIM_DEBUG_FS_HEADERS";

/// Inject server filesystem location into response headers — **off by default**.
///
/// Only emits the `x-server-*` headers when [`DEBUG_FS_HEADERS_ENV`] is set to a
/// truthy value; otherwise the response passes through untouched.
pub async fn fs_location(req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    if !debug_headers_enabled() {
        return res;
    }
    let loc = crate::filesystem::FsLocation::current();
    let val = serde_json::to_value(&loc).unwrap_or_default();
    inject_headers_from_value(res, val)
}

/// Whether the opt-in debug env var is set to a truthy value (`1`/`true`/`yes`,
/// case-insensitive). Absent, empty, or any other value keeps the headers off.
fn debug_headers_enabled() -> bool {
    std::env::var(DEBUG_FS_HEADERS_ENV)
        .ok()
        .map(|raw| {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Inject fields from a JSON object value as `x-*` response headers.
fn inject_headers_from_value(mut res: Response, val: serde_json::Value) -> Response {
    let map = match val {
        serde_json::Value::Object(obj) => obj,
        _ => return res,
    };
    for (key, value) in map {
        let str_value = match value {
            serde_json::Value::String(string) => string,
            _ => continue,
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
