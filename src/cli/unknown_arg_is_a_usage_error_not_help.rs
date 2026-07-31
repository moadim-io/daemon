
#[test]
fn unknown_arg_is_a_usage_error_not_help() {
    // A typo like `staus` (or any unrecognized token) must be classified as a usage error, distinct
    // from an explicit `help` request, so the dispatcher can write to stderr and exit non-zero
    // instead of printing help to stdout and exiting 0.
    assert_eq!(parse(argv(&["staus"])), Command::Usage("staus".into()));
    assert_eq!(
        parse(argv(&["--nonsense"])),
        Command::Usage("--nonsense".into())
    );
    assert_ne!(parse(argv(&["staus"])), Command::Help);
}

#[test]
fn print_usage_error_runs() {
    // Smoke-test the stderr usage-error printer: it must not panic for an arbitrary token.
    print_usage_error("staus");
}

#[test]
fn usage_exit_code_is_two() {
    // Conventional usage-error exit code, distinct from EXIT_NOT_RUNNING (3) and success (0).
    assert_eq!(EXIT_USAGE, 2);
    assert_ne!(EXIT_USAGE, EXIT_NOT_RUNNING);
}

#[test]
fn data_keywords_route_to_data_command_with_full_argv() {
    for keyword in DATA_COMMANDS {
        let args = argv(&[keyword, "list"]);
        assert_eq!(
            parse(args.clone()),
            Command::Data(args),
            "keyword {keyword}"
        );
    }
    // The keyword itself with no further args still routes to the data dispatcher (which then
    // surfaces clap's usage error), rather than the lifecycle parser.
    assert_eq!(
        parse(argv(&["routines"])),
        Command::Data(argv(&["routines"]))
    );
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
