//! Tests for CLI argument parsing and HTTP status parsing.

use std::io::{Read as _, Write as _};

use super::*;

/// Build a `Vec<String>` from string literals for [`parse`].
fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

#[test]
fn no_args_defaults_to_background() {
    assert_eq!(parse(argv(&[])), Command::Background);
}

#[test]
fn remote_bind_allowed_requires_exact_value_one() {
    let _guard = EnvGuard::set("MOADIM_ALLOW_REMOTE", "1");
    assert!(remote_bind_allowed());
}

#[test]
fn remote_bind_allowed_false_for_unset_or_other_values() {
    let previous = std::env::var_os("MOADIM_ALLOW_REMOTE");
    // SAFETY: tests in this crate run single-threaded per binary.
    unsafe {
        std::env::remove_var("MOADIM_ALLOW_REMOTE");
    }
    assert!(!remote_bind_allowed());
    for bogus in ["true", "yes", "0", ""] {
        let _guard = EnvGuard::set("MOADIM_ALLOW_REMOTE", bogus);
        assert!(!remote_bind_allowed(), "value {bogus}");
    }
    if let Some(previous) = previous {
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            std::env::set_var("MOADIM_ALLOW_REMOTE", previous);
        }
    }
}

#[test]
fn interactive_flags_select_foreground() {
    for flag in ["-i", "--interactive", "-f", "--foreground"] {
        assert_eq!(parse(argv(&[flag])), Command::Foreground, "flag {flag}");
    }
}

#[test]
fn background_flags_select_background() {
    for flag in ["-b", "--background", "-d", "--detach", "--daemon"] {
        assert_eq!(parse(argv(&[flag])), Command::Background, "flag {flag}");
    }
}

#[test]
fn stop_and_status_commands() {
    assert_eq!(
        parse(argv(&["stop"])),
        Command::Stop {
            json: false,
            quiet: false
        }
    );
    assert_eq!(
        parse(argv(&["status"])),
        Command::Status {
            json: false,
            wait_secs: None
        }
    );
}

#[test]
fn cleanup_command() {
    assert_eq!(parse(argv(&["cleanup"])), Command::Cleanup { json: false });
}

#[test]
fn json_flag_sets_machine_readable_output() {
    assert_eq!(
        parse(argv(&["status", "--json"])),
        Command::Status {
            json: true,
            wait_secs: None
        }
    );
    assert_eq!(
        parse(argv(&["cleanup", "--json"])),
        Command::Cleanup { json: true }
    );
    assert_eq!(
        parse(argv(&["stop", "--json"])),
        Command::Stop {
            json: true,
            quiet: false
        }
    );
}

#[test]
fn quiet_flag_only_applies_to_stop() {
    for flag in ["--quiet", "-q"] {
        assert_eq!(
            parse(argv(&["stop", flag])),
            Command::Stop {
                json: false,
                quiet: true
            },
            "flag {flag}"
        );
    }
    // `--quiet` and `--json` compose; order between them does not matter.
    assert_eq!(
        parse(argv(&["stop", "--json", "--quiet"])),
        Command::Stop {
            json: true,
            quiet: true
        }
    );
    assert_eq!(
        parse(argv(&["stop", "-q", "--json"])),
        Command::Stop {
            json: true,
            quiet: true
        }
    );
    // A bare `--quiet` (no subcommand) is an unknown arg, not a stop request.
    assert_eq!(parse(argv(&["--quiet"])), Command::Usage("--quiet".into()));
    assert_eq!(parse(argv(&["-q"])), Command::Usage("-q".into()));
}

// ─── Lifecycle / HTTP-client integration tests ───────────────────────────────
//
// These exercise the parts of the CLI that talk to a running server, spawn detached
// processes, and read/write the pid file. They rely on the `MOADIM_BIND_ADDR` and
// `MOADIM_HOME_OVERRIDE` seams to target an ephemeral port and a tempdir, and on the
// single-threaded test harness (`.cargo/config.toml`) so env mutation is race-free.

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A loopback port that nothing listens on, so probes fail fast with a refused connection.
const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

/// Save an env var's prior value and restore it on drop, so a test's override never leaks.
struct EnvGuard {
    /// The environment variable name being temporarily overridden.
    name: &'static str,
    /// The value present before this guard set it, restored on drop.
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// Set `name` to `value`, remembering the prior value for restoration.
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}
include!("tests_part4.rs");
