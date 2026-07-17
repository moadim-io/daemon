//! Tests for the bind-address resolution and loopback/remote-exposure policy split out of
//! `cli/mod.rs`'s `cli_bind` module.

use super::*;

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
