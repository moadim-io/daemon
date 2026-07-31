#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn json_flag_only_applies_to_its_command() {
    // A bare `--json` (no subcommand) is an unknown arg, not a status/cleanup request.
    assert_eq!(parse(argv(&["--json"])), Command::Usage("--json".into()));
    // An unrelated trailing flag does not switch on JSON output.
    assert_eq!(
        parse(argv(&["status", "--verbose"])),
        Command::Status {
            json: false,
            wait_secs: None
        }
    );
}

#[test]
fn wait_flag_only_applies_to_status() {
    // A bare `--wait` uses the default timeout.
    assert_eq!(
        parse(argv(&["status", "--wait"])),
        Command::Status {
            json: false,
            wait_secs: Some(DEFAULT_WAIT_SECS)
        }
    );
    // `--wait=SECS` uses the given timeout.
    assert_eq!(
        parse(argv(&["status", "--wait=5"])),
        Command::Status {
            json: false,
            wait_secs: Some(5)
        }
    );
    // `--wait` and `--json` compose; order does not matter.
    assert_eq!(
        parse(argv(&["status", "--json", "--wait=5"])),
        Command::Status {
            json: true,
            wait_secs: Some(5)
        }
    );
    // A malformed `--wait=` value is ignored rather than panicking or defaulting to a wait.
    assert_eq!(
        parse(argv(&["status", "--wait=nope"])),
        Command::Status {
            json: false,
            wait_secs: None
        }
    );
    // A bare `--wait` (no subcommand) is an unknown arg, not a status request.
    assert_eq!(parse(argv(&["--wait"])), Command::Usage("--wait".into()));
}

#[test]
fn restart_command() {
    assert_eq!(
        parse(argv(&["restart"])),
        Command::Restart {
            json: false,
            quiet: false,
            interactive: false
        }
    );
}

#[test]
fn install_and_uninstall_commands() {
    assert_eq!(parse(argv(&["install"])), Command::Install);
    assert_eq!(parse(argv(&["uninstall"])), Command::Uninstall);
}

#[test]
fn trigger_command_carries_the_routine_id() {
    assert_eq!(
        parse(argv(&["trigger", "abc-123"])),
        Command::Trigger {
            id: "abc-123".to_string()
        }
    );
}

#[test]
fn run_is_a_back_compat_alias_for_trigger() {
    // `run` was the original subcommand name; it stays as a hidden alias of `trigger`.
    assert_eq!(
        parse(argv(&["run", "abc-123"])),
        Command::Trigger {
            id: "abc-123".to_string()
        }
    );
}

#[test]
fn trigger_without_an_id_falls_back_to_help() {
    // Nothing to trigger without an id, so it shows usage rather than silently no-op'ing.
    assert_eq!(parse(argv(&["trigger"])), Command::Help);
    assert_eq!(parse(argv(&["run"])), Command::Help);
}

#[test]
fn logs_command_carries_the_routine_id() {
    assert_eq!(
        parse(argv(&["logs", "abc-123"])),
        Command::Logs {
            id: "abc-123".to_string()
        }
    );
}

#[test]
fn logs_without_an_id_falls_back_to_help() {
    // Nothing to print without an id, so it shows usage rather than silently no-op'ing.
    assert_eq!(parse(argv(&["logs"])), Command::Help);
}

#[test]
fn restart_rotation_line_shows_old_and_new_pid() {
    assert_eq!(
        restart_rotation_line(Some(123), 456),
        "restarted: pid 123 -> 456"
    );
}

#[test]
fn restart_rotation_line_reads_none_when_nothing_was_running() {
    assert_eq!(
        restart_rotation_line(None, 456),
        "restarted: pid none -> 456"
    );
}

#[test]
fn restart_json_reports_old_and_new_pid() {
    let value: serde_json::Value = serde_json::from_str(&restart_json(Some(123), 456)).unwrap();
    assert_eq!(value["old"], serde_json::json!(123));
    assert_eq!(value["new"], serde_json::json!(456));

    let fresh: serde_json::Value = serde_json::from_str(&restart_json(None, 456)).unwrap();
    assert!(fresh["old"].is_null());
    assert_eq!(fresh["new"], serde_json::json!(456));
}

#[test]
fn help_and_version_flags() {
    for flag in ["-h", "--help", "help"] {
        assert_eq!(parse(argv(&[flag])), Command::Help, "flag {flag}");
    }
    for flag in ["-V", "--version", "version"] {
        assert_eq!(parse(argv(&[flag])), Command::Version, "flag {flag}");
    }
}
include!("tests_part5.rs");
