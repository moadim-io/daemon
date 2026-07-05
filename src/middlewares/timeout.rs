use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// Per-request deadline applied to the REST API (`/api/v1/**`).
///
/// Bounds the blast radius of a wedged handler — e.g. blocking `crontab`/`tmux`/filesystem I/O
/// that runs with no `spawn_blocking` (#360) — so a stuck request cannot pin a connection and a
/// Tokio worker forever. Deliberately scoped to the REST API only: `/mcp` is a long-lived SSE
/// stream by design and stays outside this layer (#402).
pub(crate) const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Future returned by the middleware closure built by [`request_timeout`].
type TimeoutFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

/// Build a middleware that aborts a request running longer than `duration`, returning
/// `408 Request Timeout` instead of holding the connection (and a Tokio task) open indefinitely.
///
/// Takes `duration` as a parameter instead of hardcoding [`API_REQUEST_TIMEOUT`] directly so
/// tests can exercise the timeout path in milliseconds rather than waiting out the real deadline.
pub(crate) fn request_timeout(
    duration: Duration,
) -> impl Fn(Request, Next) -> TimeoutFuture + Clone + Send + Sync + 'static {
    move |req: Request, next: Next| {
        Box::pin(async move {
            match tokio::time::timeout(duration, next.run(req)).await {
                Ok(res) => res,
                Err(_elapsed) => StatusCode::REQUEST_TIMEOUT.into_response(),
            }
        })
    }
}

#[cfg(test)]
#[path = "timeout_tests.rs"]
mod timeout_tests;
