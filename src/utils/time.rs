use std::time::SystemTime;

use chrono::{Local, TimeZone};

/// Return current Unix time in whole seconds.
pub fn now_secs() -> u64 {
    secs_since_epoch(SystemTime::now())
}

/// Format Unix seconds `ts` as a human-readable local (this machine's timezone) timestamp,
/// e.g. `"2026-07-13 14:30:05 +0300"`. The offset is included since "local" is the *daemon
/// process's* timezone, which a remote reader can't otherwise infer.
pub(crate) fn format_local(ts: u64) -> String {
    let ts = i64::try_from(ts).unwrap_or(i64::MAX);
    Local.timestamp_opt(ts, 0).single().map_or_else(
        || "—".to_string(),
        |dt| dt.format("%Y-%m-%d %H:%M:%S %z").to_string(),
    )
}

/// Whole seconds between the Unix epoch and `moment`.
///
/// A clock that reads before 1970 — as happens on a VM or container booted
/// with a dead real-time clock — would make `duration_since` return an error.
/// We clamp those to `0` rather than panicking so the daemon stays up until
/// the clock is corrected.
fn secs_since_epoch(moment: SystemTime) -> u64 {
    moment
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod time_tests;
