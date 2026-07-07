//! Tests for `BIND_ADDR_ENV` overrides reflected in the status/stop/cleanup `--json` shapes.

use super::*;

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

#[test]
fn bind_addr_uses_default_when_unset() {
    let previous = std::env::var_os(BIND_ADDR_ENV);
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::remove_var(BIND_ADDR_ENV);
    }
    assert_eq!(bind_addr(), BIND_ADDR);
    // SAFETY: single-threaded test execution.
    unsafe {
        if let Some(value) = previous {
            std::env::set_var(BIND_ADDR_ENV, value);
        }
    }
}

#[test]
fn bind_addr_honors_override() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "127.0.0.1:6000");
    assert_eq!(bind_addr(), "127.0.0.1:6000");
}

#[test]
fn status_json_address_reflects_bind_override() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "127.0.0.1:6000");
    let value: serde_json::Value = serde_json::from_str(&status_json(true, Some(7), None)).unwrap();
    assert_eq!(value["address"], serde_json::json!("127.0.0.1:6000"));
}

#[test]
fn stop_json_address_reflects_bind_override() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "127.0.0.1:6000");
    let value: serde_json::Value = serde_json::from_str(&stop_json(true, Some(7))).unwrap();
    assert_eq!(value["address"], serde_json::json!("127.0.0.1:6000"));
}

/// `status --json` and `stop --json` advertise the same `{running,pid,address}` base contract, so a
/// client can parse either uniformly. Guard that every field in `stop` is present in `status` with
/// the same value (including the override-aware `address`) so the two shapes can't silently drift
/// apart. `status` carries additional fields (`uptime_secs`, `version`) that `stop` omits.
#[test]
fn status_and_stop_json_share_the_same_shape() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "127.0.0.1:6000");
    let status: serde_json::Value =
        serde_json::from_str(&status_json(true, Some(7), None)).unwrap();
    let stop: serde_json::Value = serde_json::from_str(&stop_json(true, Some(7))).unwrap();
    // Every key in `stop` must appear in `status` with the same value.
    for (key, val) in stop.as_object().unwrap() {
        assert_eq!(
            &status[key], val,
            "field {key} differs between status and stop"
        );
    }
}

#[test]
fn cleanup_json_address_reflects_bind_override() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "127.0.0.1:6000");
    let value: serde_json::Value = serde_json::from_str(&cleanup_json(2, 0, true)).unwrap();
    assert_eq!(value["address"], serde_json::json!("127.0.0.1:6000"));
}

/// Lock the machine-readable contract across all three `--json` commands: `status`, `stop`, and
/// `cleanup` must each surface `address`, and — since they all describe the same bound endpoint —
/// the value must be identical across all three, so the shapes can't silently drift apart again.
#[test]
fn status_stop_cleanup_json_share_the_same_address() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "127.0.0.1:6000");
    let status: serde_json::Value =
        serde_json::from_str(&status_json(true, Some(7), None)).unwrap();
    let stop: serde_json::Value = serde_json::from_str(&stop_json(true, Some(7))).unwrap();
    let cleanup: serde_json::Value = serde_json::from_str(&cleanup_json(2, 0, true)).unwrap();

    let expected = serde_json::json!("127.0.0.1:6000");
    assert!(
        status["address"].is_string(),
        "status --json must include address"
    );
    assert!(
        stop["address"].is_string(),
        "stop --json must include address"
    );
    assert!(
        cleanup["address"].is_string(),
        "cleanup --json must include address"
    );
    assert_eq!(status["address"], expected);
    assert_eq!(stop["address"], expected);
    assert_eq!(cleanup["address"], expected);
}
