//! Bind-address resolution and the loopback/remote-exposure policy, split out of `cli/mod.rs` to
//! stay under the repo's per-file line gate: this is a self-contained decision (where the server
//! binds, and whether a non-loopback bind is allowed) with no dependency on the lifecycle commands
//! that remain in `cli/mod.rs`.

/// Address the server binds to and that the client talks to.
pub const BIND_ADDR: &str = "127.0.0.1:5784";

/// Environment variable overriding [`BIND_ADDR`] (test seam): lets tests run the server and probe
/// it on an ephemeral port instead of the fixed default, so they never collide with a real daemon.
pub(crate) const BIND_ADDR_ENV: &str = "MOADIM_BIND_ADDR";

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

/// Pure decision function for the startup bind-address gate (issue #253): the unauthenticated
/// REST/MCP API must never end up reachable off-host by accident, so a non-loopback bind requires
/// an explicit opt-in (`allow_remote`, sourced from [`remote_bind_allowed`]) or startup is refused.
pub fn classify_bind(addr: &str, allow_remote: bool) -> BindDecision {
    if bind_addr_is_loopback(addr) {
        BindDecision::Loopback
    } else if allow_remote {
        BindDecision::RemoteAllowed
    } else {
        BindDecision::RemoteRefused
    }
}
