#![allow(
    clippy::missing_docs_in_private_items,
    clippy::undocumented_unsafe_blocks,
    reason = "test env guards mirror existing CLI tests"
)]

//! Tests for the bind-address resolution and loopback/remote-exposure policy split out of
//! `cli/mod.rs`'s `cli_bind` module.

use super::*;

struct EnvGuard {
    name: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let old = std::env::var(name).ok();
        unsafe { std::env::set_var(name, value) };
        Self { name, old }
    }

    fn unset(name: &'static str) -> Self {
        let old = std::env::var(name).ok();
        unsafe { std::env::remove_var(name) };
        Self { name, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(old) = &self.old {
            unsafe { std::env::set_var(self.name, old) };
        } else {
            unsafe { std::env::remove_var(self.name) };
        }
    }
}

#[test]
fn bind_addr_is_loopback_true_for_v4_and_v6_loopback() {
    assert!(bind_addr_is_loopback("127.0.0.1:5784"));
    assert!(bind_addr_is_loopback("[::1]:5784"));
}

#[test]
fn bind_addr_is_loopback_false_for_non_loopback_or_unparsable() {
    assert!(!bind_addr_is_loopback("0.0.0.0:5784"));
    assert!(!bind_addr_is_loopback("192.168.1.10:5784"));
    assert!(!bind_addr_is_loopback("not-an-address"));
}

#[test]
fn classify_bind_allows_loopback_regardless_of_opt_in() {
    for allow_remote in [false, true] {
        assert_eq!(
            classify_bind("127.0.0.1:5784", allow_remote),
            BindDecision::Loopback
        );
        assert_eq!(
            classify_bind("[::1]:5784", allow_remote),
            BindDecision::Loopback
        );
    }
}

#[test]
fn classify_bind_refuses_non_loopback_without_opt_in() {
    for addr in ["0.0.0.0:5784", "192.168.1.10:5784", "not-an-address"] {
        assert_eq!(
            classify_bind(addr, false),
            BindDecision::RemoteRefused,
            "addr {addr}"
        );
    }
}

#[test]
fn classify_bind_allows_non_loopback_with_opt_in() {
    for addr in ["0.0.0.0:5784", "192.168.1.10:5784"] {
        assert_eq!(
            classify_bind(addr, true),
            BindDecision::RemoteAllowed,
            "addr {addr}"
        );
    }
}

#[test]
fn validated_bind_addr_refuses_non_loopback_without_token_or_opt_in() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "0.0.0.0:5784");
    let _token = EnvGuard::unset(API_TOKEN_ENV);
    let _allow = EnvGuard::unset("MOADIM_ALLOW_REMOTE");

    let err = validated_bind_addr().unwrap_err();

    assert!(err.contains("refusing to bind to 0.0.0.0:5784"));
    assert!(err.contains("MOADIM_API_TOKEN"));
}

#[test]
fn validated_bind_addr_allows_non_loopback_when_token_is_configured() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, "0.0.0.0:5784");
    let _token = EnvGuard::set(API_TOKEN_ENV, "secret");
    let _allow = EnvGuard::unset("MOADIM_ALLOW_REMOTE");

    assert_eq!(validated_bind_addr().unwrap(), "0.0.0.0:5784");
}
