//! Tests for the restart/stop-and-wait lifecycle.
//!
//! These drive the graceful-shutdown, force-kill, and bail paths against a throwaway loopback
//! server and a real short-lived child process, using the `MOADIM_BIND_ADDR`/`MOADIM_HOME_OVERRIDE`
//! and restart-timeout seams. The single-threaded test harness (`.cargo/config.toml`) makes the
//! env overrides race-free.
#![allow(clippy::missing_docs_in_private_items)]

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
    fn set(name: &'static str, value: &str) -> EnvGuard {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
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

/// Create a unique tempdir for `MOADIM_HOME_OVERRIDE`.
fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-restart-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

/// A loopback server that answers `200`, optionally going quiet once a marker file appears.
struct FakeServer {
    addr: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    /// A server that answers `200` until dropped.
    fn start() -> FakeServer {
        FakeServer::start_inner(None)
    }

    /// A server that answers `200` until `down_marker` exists on disk, then stops responding.
    ///
    /// This lets a test tie "server down" to an observable event — the force-kill shim creating
    /// the marker — instead of racing a wall-clock timer against the stop-and-wait poll loop.
    fn start_until_marker(down_marker: std::path::PathBuf) -> FakeServer {
        FakeServer::start_inner(Some(down_marker))
    }

    fn start_inner(down_marker: Option<std::path::PathBuf>) -> FakeServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_loop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            while !stop_loop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf);
                        let serving = match &down_marker {
                            Some(marker) => !marker.exists(),
                            None => true,
                        };
                        if serving {
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
    let home = temp_home("kill-success");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());

    // The server answers until the force-kill fires, then goes quiet. Tying "server down" to the
    // kill event — rather than a wall-clock timer racing the poll loop — makes this deterministic.
    let down_marker = home.join("killed");
    let server = FakeServer::start_until_marker(down_marker.clone());
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "80");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "10");

    // Divert the real killer to a shim that creates the down-marker instead of signalling a PID,
    // so the server stops answering exactly when the force-kill runs.
    let script = home.join("fake-kill.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\n: > '{}'\nexit 0\n", down_marker.display()),
    )
    .expect("write kill shim");
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmod shim");
    let _kill = EnvGuard::set("MOADIM_KILL_BIN", script.to_str().unwrap());

    // A pid file must exist for the force-kill branch to run; its value is irrelevant because the
    // shim ignores it.
    std::fs::create_dir_all(crate::paths::config_dir()).expect("create config dir");
    std::fs::write(crate::paths::pid_file(), "424242").expect("write pid file");

    // The first wait times out with the server up; the force-kill shim then drops it, so the
    // post-kill wait observes it stopped and we succeed.
    stop_running_and_wait().expect("server stops after force-kill -> success");

    drop(server);
    let _ = std::fs::remove_dir_all(&home);
}

#[cfg(unix)]
#[test]
fn stop_running_and_wait_bails_when_server_never_stops() {
    let server = FakeServer::start();
    let home = temp_home("bail");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "40");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "10");
    let mut child = spawn_dummy_with_pid_file();
    // Server stays up through both waits, so the kill cannot bring the port down and we bail.
    let result = stop_running_and_wait();
    assert!(result.is_err(), "still-running server must bail");
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&home);
}

#[cfg(unix)]
#[test]
fn kill_pid_terminates_a_live_process() {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    kill_pid(child.id());
    let status = child.wait().expect("reap killed child");
    assert!(
        !status.success(),
        "force-killed process exits unsuccessfully"
    );
}

/// `MOADIM_KILL_BIN` diverts `kill_pid` away from the real killer: a shim shell script records
/// that it was invoked (proving the seam fired) and a never-spawned victim PID is never signalled.
#[cfg(unix)]
#[test]
fn kill_pid_honors_kill_bin_override() {
    let dir = temp_home("kill-bin-seam");
    let marker = dir.join("ran.txt");
    let script = dir.join("fake-kill.sh");
    // Shim records its args and exits 0 — it never signals any process.
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\nexit 0\n",
            marker.display()
        ),
    )
    .expect("write shim");
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmod shim");

    let _kill = EnvGuard::set("MOADIM_KILL_BIN", script.to_str().unwrap());
    // A PID that does not exist: if the real `kill` ran it would error, but we never invoke it.
    kill_pid(424242);

    let recorded = std::fs::read_to_string(&marker).expect("shim ran and wrote its args");
    assert!(
        recorded.contains("424242"),
        "shim received the pid, proving the override diverted the call: {recorded:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn timeout_and_poll_honor_env_overrides() {
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "25");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "5");
    assert_eq!(restart_timeout(), Duration::from_millis(25));
    assert_eq!(poll_interval(), Duration::from_millis(5));
}

#[test]
fn timeout_and_poll_fall_back_to_defaults() {
    // An unparseable value falls back to the compiled default.
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "not-a-number");
    assert_eq!(restart_timeout(), RESTART_TIMEOUT);
    // An unset value also falls back.
    let previous = std::env::var_os("MOADIM_RESTART_POLL_MS");
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::remove_var("MOADIM_RESTART_POLL_MS");
    }
    assert_eq!(poll_interval(), POLL_INTERVAL);
    // SAFETY: single-threaded test execution.
    unsafe {
        if let Some(value) = previous {
            std::env::set_var("MOADIM_RESTART_POLL_MS", value);
        }
    }
}
