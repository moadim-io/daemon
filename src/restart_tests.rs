//! Tests for the restart/stop-and-wait lifecycle.
//!
//! These drive the graceful-shutdown, force-kill, and bail paths against a throwaway loopback
//! server and a real short-lived child process, using the `MOADIM_BIND_ADDR`/`MOADIM_HOME_OVERRIDE`
//! and restart-timeout seams. The single-threaded test harness (`.cargo/config.toml`) makes the
//! env overrides race-free.
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A loopback port nothing listens on, so probes fail fast.
const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

/// Save and restore an env var around a test.
struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
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

/// Create a unique tempdir for `MOADIM_HOME_OVERRIDE`.
fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-restart-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

/// A loopback server that answers `200` while alive and drops connections once not alive.
struct FakeServer {
    addr: String,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let alive = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let alive_loop = Arc::clone(&alive);
        let stop_loop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            while !stop_loop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0_u8; 1024];
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

    fn stop_after(&self, delay: Duration) {
        let alive = Arc::clone(&self.alive);
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            alive.store(false, Ordering::SeqCst);
        });
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

/// Spawn a long-lived child process and record its pid in the (overridden) pid file.
#[cfg(unix)]
fn spawn_dummy_with_pid_file() -> std::process::Child {
    let child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    std::fs::create_dir_all(crate::paths::config_dir()).expect("create config dir");
    std::fs::write(crate::paths::pid_file(), child.id().to_string()).expect("write pid file");
    child
}

#[test]
fn stop_running_and_wait_returns_ok_when_nothing_is_running() {
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", UNREACHABLE_ADDR);
    stop_running_and_wait().expect("no server -> immediate success");
}

#[cfg(unix)]
#[test]
fn stop_running_and_wait_force_kills_then_succeeds_when_server_goes_down() {
    let server = FakeServer::start();
    let home = temp_home("kill-success");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "300");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "15");
    let mut child = spawn_dummy_with_pid_file();
    // The first wait (300ms) times out with the server still up, then the server is taken down
    // at 450ms — well inside the post-kill wait's window — so that wait observes it stopped.
    // The ~150ms of slack on each side of the post-kill deadline keeps the test off the timing
    // knife-edge it used to sit on (80ms timeout / 130ms drop left only ~35ms of margin, which
    // a coverage-instrumented or otherwise loaded CI run could blow, flaking this assertion).
    server.stop_after(Duration::from_millis(450));
    stop_running_and_wait().expect("server stops after force-kill -> success");
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&home);
}
include!("stop_running_and_wait_bails_when_server_never_stops_tests.rs");
