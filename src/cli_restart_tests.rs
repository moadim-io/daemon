#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn restart_command_interactive_flag() {
    for flag in ["-i", "--interactive"] {
        assert_eq!(
            parse(argv(&["restart", flag])),
            Command::Restart {
                json: false,
                quiet: false,
                interactive: true
            },
            "flag {flag} should select interactive restart"
        );
    }
}

#[test]
fn restart_json_and_quiet_flags_parse() {
    assert_eq!(
        parse(argv(&["restart", "--json"])),
        Command::Restart {
            json: true,
            quiet: false,
            interactive: false
        }
    );
    for flag in ["--quiet", "-q"] {
        assert_eq!(
            parse(argv(&["restart", flag])),
            Command::Restart {
                json: false,
                quiet: true,
                interactive: false
            },
            "flag {flag}"
        );
    }
    assert_eq!(
        parse(argv(&["restart", "--json", "-q"])),
        Command::Restart {
            json: true,
            quiet: true,
            interactive: false
        }
    );
}

#[test]
fn restart_json_reports_old_new_pid_and_address() {
    let rotated: serde_json::Value = serde_json::from_str(&restart_json(Some(123), 456)).unwrap();
    assert_eq!(rotated["old"], 123);
    assert_eq!(rotated["new"], 456);
    assert_eq!(rotated["address"], bind_addr());

    // `old` is null when nothing was running, mirroring the `none` rotation rendering.
    let fresh: serde_json::Value = serde_json::from_str(&restart_json(None, 456)).unwrap();
    assert!(fresh["old"].is_null());
    assert_eq!(fresh["new"], 456);
}
