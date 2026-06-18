use std::time::SystemTime;

/// Return current Unix time in whole seconds.
pub fn now_secs() -> u64 {
    secs_since_epoch(SystemTime::now())
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
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod time_tests;
