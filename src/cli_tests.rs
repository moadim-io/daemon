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
    assert_eq!(parse(argv(&["status"])), Command::Status);
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
    assert_eq!(parse_status_code("HTTP/1.1 503 Service Unavailable"), Some(503));
}

#[test]
fn rejects_malformed_status_line() {
    assert_eq!(parse_status_code(""), None);
    assert_eq!(parse_status_code("garbage"), None);
}
