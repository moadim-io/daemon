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
    assert_eq!(parse(argv(&["--quiet"])), Command::Help);
    assert_eq!(parse(argv(&["-q"])), Command::Help);
}

#[test]
fn json_flag_only_applies_to_its_command() {
    // A bare `--json` (no subcommand) is an unknown arg, not a status/cleanup request.
    assert_eq!(parse(argv(&["--json"])), Command::Help);
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
    assert_eq!(parse(argv(&["--wait"])), Command::Help);
}

#[test]
fn status_json_reports_running_pid_and_address() {
    let health = HealthInfo {
        uptime_secs: 8123,
        version: "1.2.3".to_string(),
    };
    let value: serde_json::Value =
        serde_json::from_str(&status_json(true, Some(42), Some(health))).unwrap();
    assert_eq!(value["running"], serde_json::json!(true));
    assert_eq!(value["pid"], serde_json::json!(42));
    assert_eq!(value["address"], serde_json::json!(BIND_ADDR));
    assert_eq!(value["uptime_secs"], serde_json::json!(8123));
    assert_eq!(value["version"], serde_json::json!("1.2.3"));
}

#[test]
fn status_json_null_pid_when_unknown_or_down() {
    let value: serde_json::Value = serde_json::from_str(&status_json(false, None, None)).unwrap();
    assert_eq!(value["running"], serde_json::json!(false));
    assert!(value["pid"].is_null());
    assert_eq!(value["address"], serde_json::json!(BIND_ADDR));
    // Server-sourced fields are null when no /health was folded in.
    assert!(value["uptime_secs"].is_null());
    assert!(value["version"].is_null());
}

#[test]
fn parse_health_reads_uptime_and_version() {
    let body = r#"{"status":"ok","uptime_secs":42,"running":true,"version":"9.9.9"}"#;
    assert_eq!(
        parse_health(body),
        Some(HealthInfo {
            uptime_secs: 42,
            version: "9.9.9".to_string(),
        })
    );
}

#[test]
fn parse_health_rejects_malformed_or_incomplete_bodies() {
    // Not JSON at all.
    assert_eq!(parse_health("not json"), None);
    // Missing version.
    assert_eq!(parse_health(r#"{"uptime_secs":1}"#), None);
    // Missing uptime_secs.
    assert_eq!(parse_health(r#"{"version":"1.0.0"}"#), None);
    // Wrong types.
    assert_eq!(
        parse_health(r#"{"uptime_secs":"x","version":"1.0.0"}"#),
        None
    );
}

#[test]
fn fetch_health_parses_a_well_formed_health_response() {
    let server = FakeServer::start(
        200,
        r#"{"status":"ok","uptime_secs":7,"running":true,"version":"3.2.1"}"#.to_string(),
    );
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(
        fetch_health(),
        Some(HealthInfo {
            uptime_secs: 7,
            version: "3.2.1".to_string(),
        })
    );
}

#[test]
fn fetch_health_is_none_on_non_200_status() {
    let server = FakeServer::start(503, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(fetch_health(), None);
}

#[test]
fn fetch_health_is_none_when_no_server() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(fetch_health(), None);
}

#[test]
fn wait_until_returns_true_immediately_when_check_already_passes() {
    assert!(wait_until(|| true, Duration::from_secs(0)));
}

#[test]
fn wait_until_calls_check_at_least_once_even_with_zero_timeout() {
    let calls = std::cell::Cell::new(0);
    let succeeded = wait_until(
        || {
            calls.set(calls.get() + 1);
            false
        },
        Duration::from_secs(0),
    );
    assert!(!succeeded);
    assert_eq!(calls.get(), 1);
}

#[test]
fn wait_until_polls_until_check_flips_true() {
    let calls = std::cell::Cell::new(0);
    let succeeded = wait_until(
        || {
            calls.set(calls.get() + 1);
            calls.get() >= 3
        },
        Duration::from_secs(5),
    );
    assert!(succeeded);
    assert_eq!(calls.get(), 3);
}

#[test]
fn cleanup_json_reports_removed_and_running() {
    let value: serde_json::Value = serde_json::from_str(&cleanup_json(3, true)).unwrap();
    assert_eq!(value["running"], serde_json::json!(true));
    assert_eq!(value["removed"], serde_json::json!(3));
    assert_eq!(value["address"], serde_json::json!(BIND_ADDR));

    let down: serde_json::Value = serde_json::from_str(&cleanup_json(0, false)).unwrap();
    assert_eq!(down["running"], serde_json::json!(false));
    assert_eq!(down["removed"], serde_json::json!(0));
    assert_eq!(down["address"], serde_json::json!(BIND_ADDR));
}

#[test]
fn stop_json_reports_running_pid_and_address() {
    let up: serde_json::Value = serde_json::from_str(&stop_json(true, Some(42))).unwrap();
    assert_eq!(up["running"], serde_json::json!(true));
    assert_eq!(up["pid"], serde_json::json!(42));
    assert_eq!(up["address"], serde_json::json!(BIND_ADDR));

    let down: serde_json::Value = serde_json::from_str(&stop_json(false, None)).unwrap();
    assert_eq!(down["running"], serde_json::json!(false));
    assert!(down["pid"].is_null());
    assert_eq!(down["address"], serde_json::json!(BIND_ADDR));
}

/// Collect the top-level object keys of a JSON document into an order-independent set.
fn json_key_set(json: &str) -> std::collections::BTreeSet<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn status_and_stop_json_share_a_common_key_set() {
    // `status --json` and `stop --json` share a common `{running,pid,address}` base so consumers
    // can parse either uniformly; `status` additionally folds in server-sourced `uptime_secs`/
    // `version` (see `status_and_stop_json_share_the_same_shape`, which guards the shared fields'
    // *values*). Here we guard the key *sets*: every key `stop` emits must also appear in `status`,
    // for both the running and the down/null-pid branches, so a key can't be dropped from one side
    // without the drift being caught.
    assert!(
        json_key_set(&stop_json(true, Some(42))).is_subset(&json_key_set(&status_json(
            true,
            Some(42),
            None
        ))),
        "every key in stop --json must also appear in status --json (running branch)"
    );
    assert!(
        json_key_set(&stop_json(false, None))
            .is_subset(&json_key_set(&status_json(false, None, None))),
        "every key in stop --json must also appear in status --json (down branch)"
    );
}

#[test]
fn liveness_exit_code_maps_running_to_codes() {
    // A reachable server exits 0; a missing one exits the documented EXIT_NOT_RUNNING.
    assert_eq!(liveness_exit_code(true), 0);
    assert_eq!(liveness_exit_code(false), EXIT_NOT_RUNNING);
    assert_eq!(EXIT_NOT_RUNNING, 3);
}

#[test]
fn restart_command() {
    assert_eq!(parse(argv(&["restart"])), Command::Restart { json: false });
}

#[test]
fn restart_command_with_json_flag() {
    assert_eq!(
        parse(argv(&["restart", "--json"])),
        Command::Restart { json: true }
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
fn unknown_arg_falls_back_to_help() {
    assert_eq!(parse(argv(&["--nonsense"])), Command::Help);
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
    let value: serde_json::Value = serde_json::from_str(&cleanup_json(2, true)).unwrap();
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
    let cleanup: serde_json::Value = serde_json::from_str(&cleanup_json(2, true)).unwrap();

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
