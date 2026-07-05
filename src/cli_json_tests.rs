//! Tests for JSON shape, spawn, and coverage paths.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
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

fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-cli-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

struct FakeServer {
    addr: String,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    fn start(status: u16, body: String) -> Self {
        Self::start_with_liveness(status, body, true)
    }

    fn start_after(status: u16, body: String, delay: Duration) -> Self {
        let server = Self::start_with_liveness(status, body, false);
        let alive = Arc::clone(&server.alive);
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            alive.store(true, Ordering::SeqCst);
        });
        server
    }

    fn start_with_liveness(status: u16, body: String, initial_alive: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let alive = Arc::new(AtomicBool::new(initial_alive));
        let stop = Arc::new(AtomicBool::new(false));
        let alive_loop = Arc::clone(&alive);
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
                        if alive_loop.load(Ordering::SeqCst) {
                            let _ = stream.write_all(response.as_bytes());
                        }
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
            alive,
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

// ─── README `--json` shape drift guard ─────────────────────────────────────────
//
// The README documents the exact `--json` object shape for `status`/`cleanup`/`stop` as a
// script-facing stability promise (see the "Scripting" table). Nothing previously pinned those
// documented key sets to the *actual* keys the `*_json` formatters emit, so a field renamed, added,
// or removed in code (or in the README) could drift silently. The tests below parse the documented
// shape literal straight out of README.md and assert it names exactly the same keys the formatter
// produces; the exit-code half of the same contract is already locked by
// `status_reports_down_when_no_server`/`status_reports_running_with_pid` and their `stop`/`cleanup`
// counterparts.

/// Return the top-level object keys named by a `--json` shape literal, e.g. turn
/// `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` into `["running", "pid", "address"]`.
/// The shapes documented in README.md never nest an object/array or embed a comma inside a string
/// value, so splitting on top-level commas and taking each field's pre-colon, quote-trimmed key is
/// sufficient (no JSON parser needed).
fn shape_keys(shape: &str) -> Vec<String> {
    shape
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split(',')
        .map(|field| {
            field
                .split(':')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .collect()
}

/// Extract the documented `--json` shape literal (the `{...}` text) from the README "Scripting"
/// table row whose first cell is `` `moadim <command> --json` ``.
fn readme_json_shape(command: &str) -> String {
    let readme = include_str!("../README.md");
    let marker = format!("`moadim {command} --json`");
    let line = readme
        .lines()
        .find(|line| line.contains(&marker))
        .unwrap_or_else(|| panic!("README scripting table has no row for {marker}"));
    let start = line.find('{').expect("shape literal starts with `{`");
    let end = line[start..]
        .find('}')
        .map(|offset| start + offset)
        .expect("shape literal ends with `}`");
    line[start..=end].to_string()
}

/// Sorted object keys of a `--json` formatter's output, for order-independent comparison against
/// [`shape_keys`].
fn actual_keys(json: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(json).expect("formatter emits valid JSON");
    let mut keys: Vec<String> = value
        .as_object()
        .expect("formatter emits a JSON object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

#[test]
fn readme_status_json_shape_matches_actual_keys() {
    let mut documented = shape_keys(&readme_json_shape("status"));
    documented.sort();
    let health = HealthInfo {
        uptime_secs: 42,
        version: "0.1.0".to_string(),
    };
    assert_eq!(
        documented,
        actual_keys(&status_json(true, Some(7), Some(health))),
        "README `moadim status --json` shape has drifted from status_json's actual keys"
    );
}

#[test]
fn readme_cleanup_json_shape_matches_actual_keys() {
    let mut documented = shape_keys(&readme_json_shape("cleanup"));
    documented.sort();
    assert_eq!(
        documented,
        actual_keys(&cleanup_json(3, 12345, true)),
        "README `moadim cleanup --json` shape has drifted from cleanup_json's actual keys"
    );
}

#[test]
fn readme_stop_json_shape_matches_actual_keys() {
    let mut documented = shape_keys(&readme_json_shape("stop"));
    documented.sort();
    assert_eq!(
        documented,
        actual_keys(&stop_json(true, Some(7))),
        "README `moadim stop --json` shape has drifted from stop_json's actual keys"
    );
}

#[test]
fn print_help_and_version_emit_without_panicking() {
    print_help();
    print_version();
}

#[test]
fn stop_reports_not_running_when_no_server() {
    let home = temp_home("stop-down");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(stop(false, false).unwrap(), EXIT_NOT_RUNNING);
    assert_eq!(stop(true, false).unwrap(), EXIT_NOT_RUNNING);
    // --quiet suppresses the human line but keeps the exit-code contract.
    assert_eq!(stop(false, true).unwrap(), EXIT_NOT_RUNNING);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn stop_signals_running_server() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("stop-up");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(stop(false, false).unwrap(), 0);
    assert_eq!(stop(true, false).unwrap(), 0);
    // --quiet suppresses the human line but keeps the success exit code.
    assert_eq!(stop(false, true).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn stop_errors_on_unexpected_status() {
    let server = FakeServer::start(500, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(stop(false, false).is_err());
}

#[test]
fn status_reports_down_when_no_server() {
    let home = temp_home("status-down");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(status(false, None).unwrap(), EXIT_NOT_RUNNING);
    assert_eq!(status(true, None).unwrap(), EXIT_NOT_RUNNING);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn status_reports_running_with_pid() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("status-up");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    // A pid file makes the human-readable "running (pid N)" suffix branch run.
    write_pid_file().unwrap();
    assert_eq!(status(false, None).unwrap(), 0);
    assert_eq!(status(true, None).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn status_wait_times_out_when_server_never_comes_up() {
    let home = temp_home("status-wait-timeout");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    // Zero seconds still probes once before giving up, so this returns promptly.
    assert_eq!(status(false, Some(0)).unwrap(), EXIT_NOT_RUNNING);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn status_wait_succeeds_once_server_comes_up() {
    let server = FakeServer::start_after(200, String::new(), Duration::from_millis(100));
    let home = temp_home("status-wait-success");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    // The first probe (no `--wait`) misses since the server isn't up yet...
    assert_eq!(status(false, None).unwrap(), EXIT_NOT_RUNNING);
    // ...but `--wait` polls past the 100ms delay and observes it come up.
    assert_eq!(status(false, Some(5)).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn cleanup_reports_removed_counts_when_running() {
    let home = temp_home("cleanup-up");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    // Singular count exercises the "" plural branch.
    {
        let server = FakeServer::start(200, "{\"removed\":1}".to_string());
        let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
        assert_eq!(cleanup(false).unwrap(), 0);
        assert_eq!(cleanup(true).unwrap(), 0);
    }
    // Plural count exercises the "es" plural branch.
    {
        let server = FakeServer::start(200, "{\"removed\":2}".to_string());
        let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
        assert_eq!(cleanup(false).unwrap(), 0);
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn cleanup_reports_not_running_when_no_server() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(cleanup(false).unwrap(), EXIT_NOT_RUNNING);
    assert_eq!(cleanup(true).unwrap(), EXIT_NOT_RUNNING);
}

#[test]
fn cleanup_errors_on_unexpected_status() {
    let server = FakeServer::start(500, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(cleanup(false).is_err());
}

#[test]
fn trigger_triggers_routine_when_server_responds() {
    let server = FakeServer::start(200, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(trigger("some-id".to_string()).unwrap(), 0);
}

#[test]
fn trigger_reports_unknown_routine_on_404() {
    // A 404 from the trigger route means no routine has that id — a user error, surfaced as a
    // non-zero exit via the bubbled `Err`, distinct from "server not running".
    let server = FakeServer::start(404, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(trigger("missing".to_string()).is_err());
}

#[test]
fn trigger_errors_on_unexpected_status() {
    let server = FakeServer::start(500, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(trigger("some-id".to_string()).is_err());
}

#[test]
fn trigger_reports_not_running_when_no_server() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(trigger("some-id".to_string()).unwrap(), EXIT_NOT_RUNNING);
}
