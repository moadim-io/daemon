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
    assert!(
        content.contains("schedule.compailed.cron"),
        "gitignore must cover cron-union output"
    );
    assert!(
        content.contains(".compailed.cron"),
        "gitignore must keep covering legacy cron-union output"
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
    assert!(
        content.contains("schedule.compailed.cron"),
        "missing cron-union pattern must be re-added"
    );
    assert!(
        content.contains(".compailed.cron"),
        "missing legacy cron-union pattern must be re-added"
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

// ─── Additional coverage tests ────────────────────────────────────────────────

// Filesystem-blocked and timeout error-path tests for write_pid_file/spawn_detached/
// run_background/restart live in cli_spawn_error_tests.rs.

// `docs/moadim.1` hand-mirrors the CLI and hardcodes its own version in the `.TH` header
// (e.g. `"moadim 0.16.0"`). Nothing previously kept that in lockstep with `Cargo.toml`, so a
// release could silently ship a man page reporting the *previous* version (issue #556). Fail
// loudly on drift instead.

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "spawn_tests_part2.rs"]
mod spawn_tests_part2;
