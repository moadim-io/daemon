//! Bind-address resolution and the loopback/remote-exposure policy, split out of `cli/mod.rs` to
//! stay under the repo's per-file line gate: this is a self-contained decision (where the server
//! binds, and whether a non-loopback bind is allowed) with no dependency on the lifecycle commands
//! that remain in `cli/mod.rs`.

/// Address the server binds to and that the client talks to.
pub const BIND_ADDR: &str = "127.0.0.1:5784";

/// Environment variable overriding [`BIND_ADDR`] (test seam): lets tests run the server and probe
/// it on an ephemeral port instead of the fixed default, so they never collide with a real daemon.
pub(crate) const BIND_ADDR_ENV: &str = "MOADIM_BIND_ADDR";

/// Environment variable holding the optional shared-secret API token. When set, first-party CLI
/// requests send it as `Authorization: Bearer ***` and the server enforces it on REST/MCP.
pub(crate) const API_TOKEN_ENV: &str = "MOADIM_API_TOKEN";

/// Return the configured API token, trimming surrounding whitespace and treating blank as disabled.
pub(crate) fn api_token() -> Option<String> {
    std::env::var(API_TOKEN_ENV)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

/// Whether API/MCP authentication is enabled for this process.
pub fn api_token_configured() -> bool {
    api_token().is_some()
}

/// The socket address to bind/probe, honoring the [`BIND_ADDR_ENV`] override when set.
pub fn bind_addr() -> String {
    std::env::var(BIND_ADDR_ENV).unwrap_or_else(|_| BIND_ADDR.to_string())
}

/// Returns `true` if `addr` (as returned by [`bind_addr`]) resolves to a loopback interface.
///
/// The REST/MCP API has no authentication (issue #504): binding to a non-loopback address
/// exposes unauthenticated routine CRUD to the network. An address this can't parse is treated
/// as non-loopback so callers warn rather than stay silent.
pub fn bind_addr_is_loopback(addr: &str) -> bool {
    addr.parse::<std::net::SocketAddr>()
        .is_ok_and(|socket| socket.ip().is_loopback())
}

/// Environment variable that opts into binding [`bind_addr`] to a non-loopback address. Must be
/// set to exactly `"1"`; anything else (unset, `"true"`, `"yes"`, …) is treated as not opted in,
/// so a typo fails closed instead of silently exposing the unauthenticated API (issue #253).
const ALLOW_REMOTE_ENV: &str = "MOADIM_ALLOW_REMOTE";

/// Returns `true` if the operator has explicitly opted into a non-loopback bind via
/// [`ALLOW_REMOTE_ENV`].
pub fn remote_bind_allowed() -> bool {
    std::env::var(ALLOW_REMOTE_ENV).as_deref() == Ok("1")
}

/// The outcome of checking a resolved bind address against the loopback/opt-in policy, decided by
/// [`classify_bind`].
#[derive(Debug, PartialEq, Eq)]
pub enum BindDecision {
    /// `addr` is loopback-only; no warning needed, start normally.
    Loopback,
    /// `addr` is not loopback, but [`ALLOW_REMOTE_ENV`] is set; start, but the caller should log a
    /// prominent warning first.
    RemoteAllowed,
    /// `addr` is not loopback and [`ALLOW_REMOTE_ENV`] is not set; the caller must refuse to
    /// start rather than silently exposing the unauthenticated API.
    RemoteRefused,
}

/// Pure decision function for the startup bind-address gate (issues #253/#504): REST/MCP must
/// never end up reachable off-host by accident. A non-loopback bind requires either an explicit
/// legacy/dev opt-in (`allow_remote`) or a configured API token.
pub fn classify_bind(addr: &str, allow_remote: bool) -> BindDecision {
    if bind_addr_is_loopback(addr) {
        BindDecision::Loopback
    } else if allow_remote {
        BindDecision::RemoteAllowed
    } else {
        BindDecision::RemoteRefused
    }
}

/// Resolve the daemon bind address and refuse accidental unauthenticated non-loopback exposure.
pub fn validated_bind_addr() -> Result<String, String> {
    let addr = bind_addr();
    let allow_remote = remote_bind_allowed() || api_token_configured();
    match classify_bind(&addr, allow_remote) {
        BindDecision::Loopback | BindDecision::RemoteAllowed => Ok(addr),
        BindDecision::RemoteRefused => Err(format!(
            "refusing to bind to {addr}: it is not loopback-only and MOADIM_API_TOKEN is not set. \
             Set MOADIM_API_TOKEN to protect REST/MCP with a bearer token, or set \
             MOADIM_ALLOW_REMOTE=1 to start without auth if you understand and accept the RCE risk."
        )),
    }
}
