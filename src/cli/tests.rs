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

/// A throwaway loopback HTTP server for driving the CLI's probe/signal client.
/// It answers every connection with a canned status line and body.
struct FakeServer {
    /// The `host:port` the server is listening on, for `MOADIM_BIND_ADDR`.
    addr: String,
    /// Signals the accept loop to exit.
    stop: Arc<AtomicBool>,
    /// The accept-loop thread handle, joined on drop.
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    /// Start a server on an ephemeral port answering with `status` and `body`.
    fn start(status: u16, body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_loop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            while !stop_loop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf);
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            addr,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[path = "query_tests.rs"]
mod cli_query_tests;

#[path = "bind_override_tests.rs"]
mod cli_bind_override_tests;

#[path = "restart_tests.rs"]
mod cli_restart_tests;
