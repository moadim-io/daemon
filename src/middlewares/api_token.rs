use crate::error::AppError;
use axum::{
    extract::Request,
    http::{header, HeaderMap},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Header accepted as a browser/CLI-friendly alternative to `Authorization: Bearer ***`.
pub(crate) const TOKEN_HEADER: &str = "x-moadim-token";

/// Environment variable holding the shared API token. When unset or blank, auth is disabled.
pub(crate) const API_TOKEN_ENV: &str = "MOADIM_API_TOKEN";

/// Return the configured API token, trimming surrounding whitespace and treating blank as disabled.
pub fn api_token() -> Option<String> {
    std::env::var(API_TOKEN_ENV)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

/// True when a header set carries the configured token in either supported form.
fn authorized(headers: &HeaderMap, token: &str) -> bool {
    bearer_token(headers).is_some_and(|candidate| candidate == token)
        || header_token(headers).is_some_and(|candidate| candidate == token)
}

/// Extract `Authorization: Bearer ***` when present and valid UTF-8.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    value.strip_prefix("Bearer ").map(str::trim)
}

/// Extract `X-Moadim-Token: <token>` when present and valid UTF-8.
fn header_token(headers: &HeaderMap) -> Option<&str> {
    headers.get(TOKEN_HEADER)?.to_str().ok().map(str::trim)
}

/// Middleware protecting REST and MCP surfaces when `MOADIM_API_TOKEN` is configured.
///
/// Missing, malformed, or wrong credentials fail closed with `401`. When no token is configured the
/// middleware is a pass-through so default loopback-only installs keep today's zero-config behavior.
pub async fn api_token_auth(req: Request, next: Next) -> Response {
    let Some(token) = api_token() else {
        return next.run(req).await;
    };
    if authorized(req.headers(), &token) {
        next.run(req).await
    } else {
        AppError::Unauthorized("missing or invalid API token".to_string()).into_response()
    }
}

#[cfg(test)]
#[path = "api_token_tests.rs"]
mod api_token_tests;
