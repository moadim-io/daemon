//! Dev-tunnel host suffixes from `machine.local.toml` (`tunnel_host_suffixes`): read per request and
//! validated at load, so only domains with at least two labels reach the `Host` check.

use super::parse_machine_toml;
use crate::paths::machine_config_path;

/// Public dev-tunnel domain suffixes from `machine.local.toml` (`tunnel_host_suffixes`), or empty
/// when the file is absent, unparsable, or the key has the wrong type. Entries that fail
/// [`is_valid_tunnel_suffix`] are dropped, so a one-label entry such as `com` never reaches the
/// host check. Read from disk on each call with a non-blocking read, so an edit applies to the next
/// request and the async middleware never blocks its executor thread.
pub async fn tunnel_host_suffixes() -> Vec<String> {
    let text = tokio::fs::read_to_string(machine_config_path()).await.ok();
    parse_machine_toml(text)
        .tunnel_host_suffixes
        .into_iter()
        .filter(|suffix| is_valid_tunnel_suffix(suffix))
        .collect()
}

/// `true` when `suffix` is a domain a tunnel host may safely end with: at least two labels, each
/// non-empty and made only of ASCII letters, digits or `-`. One leading `.` is allowed. Anything
/// else fails closed, with no normalization: a `*.` prefix, a trailing dot, whitespace, an empty
/// entry, or a bare TLD such as `com` (which would otherwise trust every `*.com` host).
pub(crate) fn is_valid_tunnel_suffix(suffix: &str) -> bool {
    let body = suffix.strip_prefix('.').unwrap_or(suffix);
    let labels: Vec<&str> = body.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}
