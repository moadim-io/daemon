//! Trust for public dev-tunnel hostnames such as Cloudflare Quick Tunnel's `*.trycloudflare.com`.
//!
//! A quick tunnel gives every run a new random subdomain, so an exact `Host` entry cannot follow
//! it. The operator names the tunnel's domain suffix once in `machine.local.toml` instead
//! (`tunnel_host_suffixes`). Only a strict subdomain of a listed suffix passes, so the bare
//! suffix and look-alike such as `eviltrycloudflare.com` stay rejected.

/// `true` when `host` (a `Host` header value, optionally `name:port`) is a strict subdomain of one
/// of `suffixes`. Comparison ignores ASCII case and a leading `.` on each configured suffix.
pub(crate) fn is_tunnel_host(host: &str, suffixes: &[String]) -> bool {
    let name = host
        .split_once(':')
        .map_or(host, |(name, _)| name)
        .to_ascii_lowercase();
    suffixes.iter().any(|suffix| {
        let suffix = suffix.trim().trim_start_matches('.').to_ascii_lowercase();
        if suffix.is_empty() {
            return false;
        }
        name.strip_suffix(suffix.as_str())
            .and_then(|rest| rest.strip_suffix('.'))
            .is_some_and(|label| !label.is_empty())
    })
}

#[cfg(test)]
#[path = "tunnel_host_tests.rs"]
mod tunnel_host_tests;
