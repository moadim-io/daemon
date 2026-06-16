//! Tests for CLI argument parsing and HTTP status parsing.

use super::*;

/// Build a `Vec<String>` from string literals for [`parse`].
fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

#[test]
fn no_args_defaults_to_background() {
    assert_eq!(parse(argv(&[])), Command::Background);
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
    assert_eq!(parse(argv(&["stop"])), Command::Stop);
    assert_eq!(parse(argv(&["status"])), Command::Status { json: false });
}

#[test]
fn cleanup_command() {
    assert_eq!(parse(argv(&["cleanup"])), Command::Cleanup { json: false });
}

#[test]
fn json_flag_sets_machine_readable_output() {
    assert_eq!(
        parse(argv(&["status", "--json"])),
        Command::Status { json: true }
    );
    assert_eq!(
        parse(argv(&["cleanup", "--json"])),
        Command::Cleanup { json: true }
    );
}

#[test]
fn json_flag_only_applies_to_its_command() {
    // A bare `--json` (no subcommand) is an unknown arg, not a status/cleanup request.
    assert_eq!(parse(argv(&["--json"])), Command::Help);
    // An unrelated trailing flag does not switch on JSON output.
    assert_eq!(
        parse(argv(&["status", "--verbose"])),
        Command::Status { json: false }
    );
}

#[test]
fn status_json_reports_running_pid_and_address() {
    let value: serde_json::Value = serde_json::from_str(&status_json(true, Some(42))).unwrap();
    assert_eq!(value["running"], serde_json::json!(true));
    assert_eq!(value["pid"], serde_json::json!(42));
    assert_eq!(value["address"], serde_json::json!(BIND_ADDR));
}

#[test]
fn status_json_null_pid_when_unknown_or_down() {
    let value: serde_json::Value = serde_json::from_str(&status_json(false, None)).unwrap();
    assert_eq!(value["running"], serde_json::json!(false));
    assert!(value["pid"].is_null());
    assert_eq!(value["address"], serde_json::json!(BIND_ADDR));
}

#[test]
fn cleanup_json_reports_removed_and_running() {
    let value: serde_json::Value = serde_json::from_str(&cleanup_json(3, true)).unwrap();
    assert_eq!(value["running"], serde_json::json!(true));
    assert_eq!(value["removed"], serde_json::json!(3));

    let down: serde_json::Value = serde_json::from_str(&cleanup_json(0, false)).unwrap();
    assert_eq!(down["running"], serde_json::json!(false));
    assert_eq!(down["removed"], serde_json::json!(0));
}

#[test]
fn restart_command() {
    assert_eq!(parse(argv(&["restart"])), Command::Restart);
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
fn help_and_version_flags() {
    for flag in ["-h", "--help", "help"] {
        assert_eq!(parse(argv(&[flag])), Command::Help, "flag {flag}");
    }
    for flag in ["-V", "--version", "version"] {
        assert_eq!(parse(argv(&[flag])), Command::Version, "flag {flag}");
    }
}

#[test]
fn unknown_arg_falls_back_to_help() {
    assert_eq!(parse(argv(&["--nonsense"])), Command::Help);
}

#[test]
fn parses_http_status_code() {
    assert_eq!(parse_status_code("HTTP/1.1 200 OK\r\n\r\n"), Some(200));
    assert_eq!(
        parse_status_code("HTTP/1.1 503 Service Unavailable"),
        Some(503)
    );
}

#[test]
fn rejects_malformed_status_line() {
    assert_eq!(parse_status_code(""), None);
    assert_eq!(parse_status_code("garbage"), None);
}

#[test]
fn extracts_body_after_headers() {
    let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"removed\":3}";
    assert_eq!(parse_body(resp), "{\"removed\":3}");
}

#[test]
fn body_is_empty_without_header_separator() {
    assert_eq!(parse_body("HTTP/1.1 200 OK"), "");
}

#[test]
fn parses_removed_count_from_cleanup_body() {
    assert_eq!(parse_removed_count("{\"removed\":0}"), Some(0));
    assert_eq!(parse_removed_count("{\"removed\":7}"), Some(7));
}

#[test]
fn rejects_non_cleanup_body() {
    assert_eq!(parse_removed_count(""), None);
    assert_eq!(parse_removed_count("not json"), None);
    assert_eq!(parse_removed_count("{\"other\":1}"), None);
}
