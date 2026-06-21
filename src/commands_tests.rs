//! Tests for the data-plane CLI subcommands.
//!
//! These drive [`run`] end to end against a throwaway loopback server (so the HTTP client path is
//! exercised) and unit-test the JSON body builders. They rely on the `MOADIM_BIND_ADDR` seam to
//! target an ephemeral port and on the single-threaded test harness so env mutation is race-free.

use super::*;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Environment variable that points the CLI's HTTP client at a chosen address.
const BIND_ENV: &str = "MOADIM_BIND_ADDR";

/// A loopback port nothing listens on, so probes fail fast with a refused connection.
const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

/// Build a `Vec<String>` argv from string literals.
fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

/// Save an env var's prior value and restore it on drop so a test's override never leaks.
struct EnvGuard {
    /// The environment variable name being temporarily overridden.
    name: &'static str,
    /// The value present before this guard set it, restored on drop.
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// Set `name` to `value`, remembering the prior value for restoration.
    fn set(name: &'static str, value: &str) -> EnvGuard {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            std::env::set_var(name, value);
        }
        EnvGuard { name, previous }
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

/// A throwaway loopback HTTP server that answers every request with a canned status and body.
struct FakeServer {
    /// The `host:port` the server is listening on, for `MOADIM_BIND_ADDR`.
    addr: String,
    /// Signals the accept loop to exit.
    stop: Arc<AtomicBool>,
    /// The accept-loop thread handle, joined on drop.
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    /// Start a server on an ephemeral port answering every connection with `status` and `body`.
    fn start(status: u16, body: &str) -> FakeServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_loop = Arc::clone(&stop);
        let response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = std::thread::spawn(move || {
            while !stop_loop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 2048];
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
        FakeServer {
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

// ─── Parse-level behavior (no server needed) ─────────────────────────────────

#[test]
fn help_and_version_return_zero() {
    assert_eq!(run(argv(&["--help"])), 0);
    assert_eq!(run(argv(&["cron-jobs", "--help"])), 0);
    assert_eq!(run(argv(&["--version"])), 0);
}

#[test]
fn usage_errors_return_two() {
    // No subcommand, an unknown subcommand, and a missing required group all map to exit 2.
    assert_eq!(run(argv(&[])), 2);
    assert_eq!(run(argv(&["nonsense"])), 2);
    assert_eq!(run(argv(&["cron-jobs"])), 2);
}

#[test]
fn invalid_json_flags_return_two_without_a_server() {
    // Body builders reject malformed JSON before any request is sent.
    assert_eq!(
        run(argv(&[
            "cron-jobs",
            "create",
            "--schedule",
            "* * * * *",
            "--handler",
            "h",
            "--metadata",
            "{bad",
        ])),
        2
    );
    assert_eq!(
        run(argv(&[
            "cron-jobs",
            "replace",
            "id",
            "--schedule",
            "* * * * *",
            "--handler",
            "h",
            "--metadata",
            "{bad",
        ])),
        2
    );
    assert_eq!(
        run(argv(&["cron-jobs", "update", "id", "--metadata", "{bad"])),
        2
    );
    assert_eq!(
        run(argv(&[
            "routines",
            "create",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
            "--repositories",
            "{bad",
        ])),
        2
    );
    assert_eq!(
        run(argv(&[
            "routines",
            "replace",
            "id",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
            "--repositories",
            "{bad",
        ])),
        2
    );
    assert_eq!(
        run(argv(&[
            "routines",
            "update",
            "id",
            "--repositories",
            "{bad"
        ])),
        2
    );
}

// ─── End-to-end dispatch against a fake server ───────────────────────────────

#[test]
fn every_subcommand_succeeds_against_a_2xx_server() {
    let server = FakeServer::start(200, "{\"ok\":true}");
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);

    let calls: &[&[&str]] = &[
        // cron jobs
        &[
            "cron-jobs",
            "create",
            "--schedule",
            "* * * * *",
            "--handler",
            "h",
        ],
        &[
            "cron-jobs",
            "create",
            "--schedule",
            "* * * * *",
            "--handler",
            "h",
            "--disabled",
            "--metadata",
            "{\"a\":1}",
        ],
        &["cron-jobs", "list"],
        &["cron", "get", "abc"],
        &[
            "cron-jobs",
            "update",
            "abc",
            "--schedule",
            "* * * * *",
            "--metadata",
            "{\"a\":1}",
            "--enabled",
            "true",
        ],
        &[
            "cron-jobs",
            "replace",
            "abc",
            "--schedule",
            "* * * * *",
            "--handler",
            "h",
        ],
        &["cron-jobs", "delete", "abc"],
        &["cron-jobs", "trigger", "abc"],
        &["cron-jobs", "logs", "abc"],
        // routines
        &[
            "routines",
            "create",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
        ],
        &[
            "routine",
            "create",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
            "--disabled",
            "--repositories",
            "[]",
        ],
        &["routines", "list"],
        &["routines", "get", "rid"],
        &[
            "routines",
            "update",
            "rid",
            "--title",
            "t2",
            "--repositories",
            "[]",
            "--enabled",
            "false",
            "--ttl-secs",
            "10",
            "--max-runtime-secs",
            "20",
        ],
        &[
            "routines",
            "replace",
            "rid",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
        ],
        &["routines", "delete", "rid"],
        &["routines", "trigger", "rid"],
        &["routines", "logs", "rid"],
        &["routines", "ical"],
        // top-level
        &["agents"],
        &["echo", "hello"],
    ];
    for call in calls {
        assert_eq!(run(argv(call)), 0, "call {call:?}");
    }
}

#[test]
fn logs_print_raw_when_body_is_not_json() {
    let server = FakeServer::start(200, "plain log line\nsecond line");
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);
    assert_eq!(run(argv(&["cron-jobs", "logs", "abc"])), 0);
}

#[test]
fn empty_body_prints_nothing_and_succeeds() {
    let server = FakeServer::start(200, "");
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);
    assert_eq!(run(argv(&["agents"])), 0);
}

#[test]
fn non_2xx_status_returns_one() {
    // A non-empty error body exercises the "print the body" branch.
    {
        let server = FakeServer::start(404, "{\"error\":\"not found\"}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["cron-jobs", "get", "missing"])), 1);
    }
    // An empty error body exercises the "skip the body" branch.
    {
        let server = FakeServer::start(500, "");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["cron-jobs", "list"])), 1);
    }
}

#[test]
fn no_server_returns_not_running_exit_code() {
    let _addr = EnvGuard::set(BIND_ENV, UNREACHABLE_ADDR);
    assert_eq!(
        run(argv(&["cron-jobs", "list"])),
        crate::cli::EXIT_NOT_RUNNING
    );
}

// ─── enable / disable ────────────────────────────────────────────────────────

#[test]
fn enable_disable_report_server_echoed_state() {
    // A 2xx whose body echoes the routine drives the "prefer the server's id/enabled" path, for
    // both states and both output modes (human line + --json object).
    {
        let server = FakeServer::start(200, "{\"id\":\"r-1\",\"enabled\":true}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "r-1"])), 0);
        assert_eq!(run(argv(&["enable", "r-1", "--json"])), 0);
    }
    {
        let server = FakeServer::start(200, "{\"id\":\"r-1\",\"enabled\":false}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["disable", "slug"])), 0);
        assert_eq!(run(argv(&["disable", "slug", "--json"])), 0);
    }
}

#[test]
fn enable_disable_fall_back_to_requested_state() {
    // A 2xx whose body lacks id/enabled (here: an empty JSON object, and a non-JSON body) exercises
    // the fallback to the addressed routine and the requested flag, for both states.
    {
        let server = FakeServer::start(200, "{}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "slug"])), 0);
        assert_eq!(run(argv(&["disable", "slug", "--json"])), 0);
    }
    {
        let server = FakeServer::start(200, "not json");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "slug"])), 0);
    }
}

#[test]
fn enable_unknown_routine_returns_one() {
    // A non-empty error body exercises the "print the body" branch...
    {
        let server = FakeServer::start(404, "{\"error\":\"not found\"}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "missing"])), 1);
    }
    // ...and an empty one the "skip the body" branch.
    {
        let server = FakeServer::start(500, "");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["disable", "missing"])), 1);
    }
}

#[test]
fn enable_without_server_returns_not_running() {
    let _addr = EnvGuard::set(BIND_ENV, UNREACHABLE_ADDR);
    assert_eq!(run(argv(&["enable", "r-1"])), crate::cli::EXIT_NOT_RUNNING);
}

// ─── Body-builder unit tests ─────────────────────────────────────────────────

#[test]
fn insert_opt_only_inserts_present_values() {
    let mut map = Map::new();
    insert_opt(&mut map, "a", Some(Value::Bool(true)));
    insert_opt(&mut map, "b", None);
    assert_eq!(map.get("a"), Some(&Value::Bool(true)));
    assert!(!map.contains_key("b"));
}

#[test]
fn object_and_to_body_build_compact_json() {
    let body = object([("message", Value::String("hi".to_string()))]);
    assert_eq!(body, "{\"message\":\"hi\"}");
}

#[test]
fn cron_body_sets_enabled_from_disabled_flag() {
    let enabled: Value =
        serde_json::from_str(&cron_body("* * * * *".into(), "h".into(), None, false).unwrap())
            .unwrap();
    assert_eq!(enabled["enabled"], Value::Bool(true));
    let disabled: Value =
        serde_json::from_str(&cron_body("* * * * *".into(), "h".into(), None, true).unwrap())
            .unwrap();
    assert_eq!(disabled["enabled"], Value::Bool(false));
}

#[test]
fn cron_body_rejects_bad_metadata() {
    assert_eq!(
        cron_body("* * * * *".into(), "h".into(), Some("{bad".into()), false),
        Err(2)
    );
}

#[test]
fn routine_body_serializes_all_fields() {
    let value: Value = serde_json::from_str(
        &routine_body(
            "* * * * *".into(),
            "title".into(),
            "agent".into(),
            "prompt".into(),
            Some("[]".into()),
            Some(30),
            Some(60),
            false,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(value["title"], Value::String("title".to_string()));
    assert_eq!(value["repositories"], Value::Array(vec![]));
    assert_eq!(value["ttl_secs"], Value::from(30));
    assert_eq!(value["enabled"], Value::Bool(true));
}

#[test]
fn routine_body_rejects_bad_repositories() {
    assert_eq!(
        routine_body(
            "* * * * *".into(),
            "t".into(),
            "a".into(),
            "p".into(),
            Some("{bad".into()),
            None,
            None,
            false,
        ),
        Err(2)
    );
}
