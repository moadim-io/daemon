use std::time::SystemTime;

/// Return current Unix time in whole seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod time_tests;
