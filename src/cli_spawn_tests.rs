use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

/// Build a `Vec<String>` from string literals for [`parse`].
fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

pub(crate) const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

pub(crate) struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    pub(crate) fn set(name: &'static str, value: &str) -> Self {
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

pub(crate) fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-cli-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

pub(crate) struct FakeServer {
    pub(crate) addr: String,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    pub(crate) fn start(status: u16, body: String) -> Self {
        Self::start_with_liveness(status, body, true)
    }

    pub(crate) fn start_with_liveness(status: u16, body: String, initial_alive: bool) -> Self {
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

#[test]
fn pid_file_write_read_clear_roundtrip() {
    let home = temp_home("pidfile");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    write_pid_file().unwrap();
    assert_eq!(read_pid_file(), Some(std::process::id()));
    let gitignore = crate::paths::config_gitignore_path();
    assert!(gitignore.exists());
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("*.local.*"),
        "gitignore must cover *.local.*"
    );
    // Manually remove one pattern; a second write must restore it without
    // duplicating the patterns already present.
    std::fs::write(&gitignore, "*.pid\n*.log\n").unwrap();
    write_pid_file().unwrap();
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("*.local.*"),
        "missing pattern must be re-added"
    );
    assert_eq!(
        content.matches("*.pid").count(),
        1,
        "existing patterns must not duplicate"
    );
    // Write a file with all patterns but no trailing newline; the next write
    // must insert the newline separator before appending (line 495 branch).
    std::fs::write(&gitignore, "*.pid\n*.log").unwrap();
    write_pid_file().unwrap();
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("*.local.*"),
        "must append after no-trailing-newline content"
    );
    // All patterns present → early return (line 492 branch). Call twice; second is a no-op.
    write_pid_file().unwrap();
    assert_eq!(
        std::fs::read_to_string(&gitignore).unwrap(),
        content,
        "no-op write must not change file"
    );
    clear_pid_file();
    assert!(read_pid_file().is_none());
    // A garbage pid file parses to None rather than panicking.
    std::fs::write(crate::paths::pid_file(), "not-a-pid").unwrap();
    assert!(read_pid_file().is_none());
    // A pid file recording a dead process (u32::MAX is never a live PID on Unix) is reconciled
    // against liveness: reported as absent and cleaned up best-effort so it doesn't linger.
    std::fs::write(crate::paths::pid_file(), u32::MAX.to_string()).unwrap();
    assert!(read_pid_file().is_none());
    assert!(!crate::paths::pid_file().exists());
    // A pid file recording a live process (this test process) reads back unchanged.
    std::fs::write(crate::paths::pid_file(), std::process::id().to_string()).unwrap();
    assert_eq!(read_pid_file(), Some(std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn write_pid_file_seeds_readmes_without_clobbering_edits() {
    let home = temp_home("pidfile-readme");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    write_pid_file().unwrap();
    let config_readme = crate::paths::config_readme_path();
    let routines_readme = crate::paths::routines_readme_path();
    let agents_readme = crate::paths::agents_readme_path();
    assert!(config_readme.exists());
    assert!(routines_readme.exists());
    assert!(agents_readme.exists());
    assert!(std::fs::read_to_string(&config_readme)
        .unwrap()
        .contains("moadim config"));
    assert!(std::fs::read_to_string(&routines_readme)
        .unwrap()
        .contains("moadim routines"));
    assert!(std::fs::read_to_string(&agents_readme)
        .unwrap()
        .contains("moadim agents"));
    // A second start must not overwrite a user's edits to any of the READMEs.
    std::fs::write(&config_readme, "custom notes").unwrap();
    std::fs::write(&routines_readme, "custom notes").unwrap();
    std::fs::write(&agents_readme, "custom notes").unwrap();
    write_pid_file().unwrap();
    assert_eq!(
        std::fs::read_to_string(&config_readme).unwrap(),
        "custom notes"
    );
    assert_eq!(
        std::fs::read_to_string(&routines_readme).unwrap(),
        "custom notes"
    );
    assert_eq!(
        std::fs::read_to_string(&agents_readme).unwrap(),
        "custom notes"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn run_background_starts_when_none_running() {
    let home = temp_home("runbg-fresh");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    run_background().unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn run_background_restarts_when_already_running() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("runbg-restart");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "2000");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "10");
    write_pid_file().unwrap();
    server.stop_after(Duration::from_millis(80));
    run_background().unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_starts_fresh_when_none_running() {
    let home = temp_home("restart-fresh");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    restart(false, false).unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_json_skips_human_text_when_none_running() {
    let home = temp_home("restart-fresh-json");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    restart(true, false).unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_replaces_running_server() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("restart-running");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "2000");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "10");
    write_pid_file().unwrap();
    server.stop_after(Duration::from_millis(80));
    restart(false, false).unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_json_reports_old_pid_when_running() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("restart-running-json");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "2000");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "10");
    write_pid_file().unwrap();
    server.stop_after(Duration::from_millis(80));
    restart(true, false).unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn foreground_already_running_message_names_pid_when_known() {
    let with_pid = foreground_already_running_message(Some(4321));
    assert!(with_pid.contains("(pid 4321)"));
    assert!(with_pid.contains("moadim stop"));
    assert!(with_pid.contains("moadim restart"));
    // With no pid file the message omits the suffix but keeps the guidance.
    let without_pid = foreground_already_running_message(None);
    assert!(!without_pid.contains("(pid"));
    assert!(without_pid.contains("refusing to start a second foreground instance"));
}

#[test]
fn foreground_preflight_refuses_when_running() {
    assert!(foreground_preflight(true, Some(7)).is_err());
    assert!(foreground_preflight(true, None).is_err());
}

#[test]
fn foreground_preflight_proceeds_when_not_running() {
    assert!(foreground_preflight(false, None).is_ok());
}

#[test]
fn ensure_not_running_for_foreground_ok_when_no_server() {
    let home = temp_home("fg-down");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _daemonized = EnvGuard::set(DAEMONIZED_ENV, "");
    // SAFETY: single-threaded test execution; clear the marker so the live-probe path runs.
    unsafe {
        std::env::remove_var(DAEMONIZED_ENV);
    }
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert!(ensure_not_running_for_foreground().is_ok());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn ensure_not_running_for_foreground_refuses_when_server_up() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("fg-up");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _daemonized = EnvGuard::set(DAEMONIZED_ENV, "");
    // SAFETY: single-threaded test execution; clear the marker so the live-probe path runs.
    unsafe {
        std::env::remove_var(DAEMONIZED_ENV);
    }
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(ensure_not_running_for_foreground().is_err());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn ensure_not_running_for_foreground_skips_for_daemonized_child() {
    // The launcher-spawned child carries MOADIM_DAEMONIZED and must be allowed to bind even while
    // the (about-to-be-replaced) server is still answering probes.
    let server = FakeServer::start(200, String::new());
    let _daemonized = EnvGuard::set(DAEMONIZED_ENV, "1");
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(ensure_not_running_for_foreground().is_ok());
}

#[test]
fn spawn_restart_launches_a_detached_helper() {
    // The helper is `current_exe --background`; under the test harness that exe is the test binary,
    // which rejects `--background` and exits immediately, so this only verifies the spawn succeeds
    // and returns a PID without leaving a real server behind.
    let home = temp_home("spawn-restart");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    let pid = spawn_restart().unwrap();
    assert!(pid > 0);
    let _ = std::fs::remove_dir_all(&home);
}

// ─── Additional coverage tests ────────────────────────────────────────────────

#[test]
fn machine_command_carries_remaining_args() {
    // Covers the `Some("machine") => Command::Machine(args[1..].to_vec())` branch.
    assert_eq!(
        parse(argv(&["machine", "show"])),
        Command::Machine(argv(&["show"]))
    );
    // "machine" alone yields an empty vec (the sub-dispatcher handles the error).
    assert_eq!(parse(argv(&["machine"])), Command::Machine(vec![]));
}

#[test]
fn parse_health_rejects_version_non_string() {
    // Covers the `.as_str()?` None arm: version is present but not a string.
    assert_eq!(parse_health(r#"{"uptime_secs":1,"version":42}"#), None);
}

// Filesystem-blocked and timeout error-path tests for write_pid_file/spawn_detached/
// run_background/restart live in cli_spawn_error_tests.rs.

/// `docs/moadim.1` hand-mirrors the CLI and hardcodes its own version in the `.TH` header
/// (e.g. `"moadim 0.16.0"`). Nothing previously kept that in lockstep with `Cargo.toml`, so a
/// release could silently ship a man page reporting the *previous* version (issue #556). Fail
/// loudly on drift instead.
#[test]
fn man_page_version_matches_cargo_pkg_version() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/moadim.1");
    let man_page = std::fs::read_to_string(path).expect("docs/moadim.1 should exist");
    let th_line = man_page
        .lines()
        .find(|line| line.starts_with(".TH MOADIM"))
        .expect("docs/moadim.1 should have a .TH header line");
    let expected = format!("\"moadim {}\"", env!("CARGO_PKG_VERSION"));
    assert!(
        th_line.contains(&expected),
        "docs/moadim.1 .TH header is stale: expected it to contain {expected:?}, got: {th_line:?}\n\
         Update the version token in docs/moadim.1 to match Cargo.toml."
    );
}
