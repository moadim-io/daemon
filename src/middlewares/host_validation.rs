use crate::error::AppError;
use axum::{
    extract::Request,
    http::{header, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::future::Future;
use std::pin::Pin;

/// Environment variable letting an operator extend [`allowed_hosts`] for a deliberate
/// remote/proxy deployment (comma-separated `host[:port]` entries), documented next to
/// `MOADIM_BIND_ADDR` in the README.
const ALLOWED_HOSTS_ENV: &str = "MOADIM_ALLOWED_HOSTS";

/// Build the allowlist of `Host`/`Origin` values [`host_validation`] trusts: `localhost` and
/// `127.0.0.1`/`[::1]` (with and without the daemon's bound port), the raw bind address itself,
/// and any operator-supplied extras from [`ALLOWED_HOSTS_ENV`].
pub(crate) fn allowed_hosts() -> Vec<String> {
    let bind = crate::cli::bind_addr();
    let port = bind.rsplit_once(':').map(|(_, port)| port);
    let mut hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "[::1]".to_string(),
        bind.clone(),
    ];
    if let Some(port) = port {
        hosts.push(format!("localhost:{port}"));
        hosts.push(format!("127.0.0.1:{port}"));
        hosts.push(format!("[::1]:{port}"));
    }
    if let Ok(extra) = std::env::var(ALLOWED_HOSTS_ENV) {
        hosts.extend(
            extra
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_string),
        );
    }
    hosts
}

/// Extract the `host[:port]` component from an `Origin` header value (`scheme://host[:port]`).
fn origin_host(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, rest)| rest)
}

/// Future returned by the middleware closure built by [`host_validation`].
type ValidationFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

/// Build a middleware guarding the unauthenticated loopback API against browser-borne
/// cross-origin abuse (issue #266):
///
/// - A request whose `Host` header is not in `allowed` is rejected with `403`. This defeats DNS
///   rebinding: a browser-borne request retargeted at the loopback socket still carries the
///   attacker's domain in `Host`, even though the TCP connection itself lands on `127.0.0.1`.
/// - A state-changing request (`POST`/`PUT`/`PATCH`/`DELETE`) whose `Origin` header names a host
///   not in `allowed` is rejected with `403`, blocking a cross-origin page from driving the
///   destructive API even without rebinding.
///
/// A request carrying **no** `Host`/`Origin` header is let through rather than rejected: every
/// real HTTP client (browsers, `curl`, the `moadim` CLI's own loopback client, the MCP transport)
/// always sends `Host`, so an absent header only happens when a test drives the `Router` directly
/// in-process without a real TCP/HTTP round trip — rejecting that would break the whole in-process
/// test suite for no security benefit. Likewise, a missing `Origin` means a non-browser client,
/// which carries no forgeable origin to check in the first place.
///
/// A `Host`/`Origin` header that **is present** but fails to parse as UTF-8 is rejected with
/// `403`, not silently let through: `HeaderValue::to_str` only rejects non-ASCII bytes, which no
/// legitimate client ever sends in these headers, so a present-but-unparseable value is treated
/// as suspicious rather than being conflated with the benign "no header at all" case above.
pub(crate) fn host_validation(
    allowed: Vec<String>,
) -> impl Fn(Request, Next) -> ValidationFuture + Clone + Send + Sync + 'static {
    move |req: Request, next: Next| {
        let allowed = allowed.clone();
        Box::pin(async move {
            match req.headers().get(header::HOST) {
                None => {}
                Some(value) => match value.to_str() {
                    Err(_) => {
                        return AppError::Forbidden("host header is not valid UTF-8".to_string())
                            .into_response();
                    }
                    Ok(host) => {
                        if !allowed.iter().any(|entry| entry.eq_ignore_ascii_case(host)) {
                            return AppError::Forbidden(format!("host '{host}' is not allowed"))
                                .into_response();
                        }
                    }
                },
            }
            let is_state_changing = matches!(
                *req.method(),
                Method::POST | Method::PUT | Method::PATCH | Method::DELETE
            );
            if is_state_changing {
                match req.headers().get(header::ORIGIN) {
                    None => {}
                    Some(value) => match value.to_str() {
                        Err(_) => {
                            return AppError::Forbidden(
                                "origin header is not valid UTF-8".to_string(),
                            )
                            .into_response();
                        }
                        Ok(origin) => {
                            let host = origin_host(origin);
                            if !allowed.iter().any(|entry| entry.eq_ignore_ascii_case(host)) {
                                return AppError::Forbidden(format!(
                                    "origin '{origin}' is not allowed"
                                ))
                                .into_response();
                            }
                        }
                    },
                }
            }
            next.run(req).await
        })
    }
}

#[cfg(test)]
#[path = "host_validation_tests.rs"]
mod host_validation_tests;
