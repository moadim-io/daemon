#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Render the `stop` result as a one-line JSON object: `{"running":bool,"pid":N|null,"address":…}`
/// — a subset of `status --json`'s shape (see `status_and_stop_json_share_a_common_key_set`).
/// `pid` is read from the pid file before the shutdown request; `address` is [`bind_addr`].
pub(crate) fn stop_json(running: bool, pid: Option<u32>) -> String {
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": bind_addr(),
    })
    .to_string()
}
