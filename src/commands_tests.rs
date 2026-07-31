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
    args.iter().map(ToString::to_string).collect()
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
    fn start(status: u16, body: &str) -> Self {
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
                        let mut buf = [0_u8; 2048];
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

// ─── Parse-level behavior (no server needed) ─────────────────────────────────

#[test]
fn help_and_version_return_zero() {
    assert_eq!(run(argv(&["--help"])), 0);
    assert_eq!(run(argv(&["routines", "--help"])), 0);
    assert_eq!(run(argv(&["--version"])), 0);
}

#[test]
fn usage_errors_return_two() {
    // No subcommand, an unknown subcommand, and a missing required group all map to exit 2.
    assert_eq!(run(argv(&[])), 2);
    assert_eq!(run(argv(&["nonsense"])), 2);
    assert_eq!(run(argv(&["routines"])), 2);
}

#[test]
fn invalid_json_flags_return_two_without_a_server() {
    // Body builders reject malformed JSON before any request is sent.
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
    // Malformed --machines JSON is rejected on the routine update path too.
    assert_eq!(
        run(argv(&["routines", "update", "id", "--machines", "{bad"])),
        2
    );
}

// ─── End-to-end dispatch against a fake server ───────────────────────────────

// ─── enable / disable ────────────────────────────────────────────────────────

// ─── Body-builder unit tests ─────────────────────────────────────────────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "commands_tests_part2.rs"]
mod commands_tests_part2;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "commands_tests_part3.rs"]
mod commands_tests_part3;
