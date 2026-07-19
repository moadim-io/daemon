//! Tests for `write_pid_file`/`spawn_detached`/`run_background`/`restart` error paths: a blocked
//! filesystem (a file occupying where a directory or the pid/log file should be) or a
//! `stop_running_and_wait` timeout, split out of `cli_spawn_tests.rs`.

use super::*;
use cli_spawn_tests::{temp_home, EnvGuard, FakeServer, UNREACHABLE_ADDR};

#[test]
fn write_pid_file_errors_when_config_dir_is_blocked() {
    // A regular file sitting where the config dir should be causes create_dir_all to fail.
    let base = temp_home("pid-dir-blocked");
    std::fs::create_dir_all(base.join(".config")).unwrap();
    std::fs::write(base.join(".config/moadim"), "block").unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    assert!(write_pid_file().is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn write_pid_file_skips_readme_when_its_subdir_is_blocked() {
    // A regular file sitting where `routines/` should be blocks that README's create_dir_all,
    // but write_pid_file is best-effort here and must still succeed overall.
    let base = temp_home("readme-subdir-blocked");
    let config_dir = base.join(".config/moadim");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("routines"), "block").unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    write_pid_file().unwrap();
    assert!(crate::paths::config_readme_path().exists());
    assert!(!crate::paths::routines_readme_path().exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn write_pid_file_errors_when_pid_path_is_directory() {
    // create_dir_all succeeds but writing the pid as a file fails because
    // a directory already occupies the pid file path.
    let base = temp_home("pid-path-is-dir");
    let config_dir = base.join(".config/moadim");
    // Create a DIRECTORY at the pid file path instead of a plain file.
    std::fs::create_dir_all(config_dir.join("moadim.pid")).unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    assert!(write_pid_file().is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn spawn_detached_errors_when_log_dir_creation_blocked() {
    // A file at the config-dir path blocks create_dir_all for the log parent dir.
    let base = temp_home("spawn-log-blocked");
    std::fs::create_dir_all(base.join(".config")).unwrap();
    std::fs::write(base.join(".config/moadim"), "block").unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    assert!(spawn_detached().is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn spawn_detached_errors_when_log_file_path_is_directory() {
    // create_dir_all for the log parent dir succeeds (the config dir exists), but
    // opening the log file fails because a directory occupies that exact path.
    let base = temp_home("spawn-log-is-dir");
    let config_dir = base.join(".config/moadim");
    // Place a DIRECTORY at daemon.log so the OpenOptions::open fails.
    std::fs::create_dir_all(config_dir.join("daemon.log")).unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    assert!(spawn_detached().is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn spawn_detached_errors_when_current_exe_resolution_fails() {
    // The `map_err` arm ahead of any filesystem work: `current_exe()` failing is otherwise
    // unreachable in a test, so this exercises it via the `MOADIM_CURRENT_EXE_FAIL_FOR_TEST` seam
    // (utils::process). `spawn_detached_with`'s `configure` closure is monomorphized per caller, so
    // `spawn_restart`'s instantiation of this same branch needs its own test below.
    let _fail = EnvGuard::set(crate::utils::process::CURRENT_EXE_FAIL_ENV, "1");
    assert!(spawn_detached().is_err());
}

#[test]
fn spawn_restart_errors_when_current_exe_resolution_fails() {
    // Same branch as above, but through `spawn_restart`'s distinct monomorphization of
    // `spawn_detached_with`.
    let _fail = EnvGuard::set(crate::utils::process::CURRENT_EXE_FAIL_ENV, "1");
    assert!(spawn_restart().is_err());
}

#[test]
fn spawn_detached_rotates_oversized_daemon_log() {
    // An existing daemon.log past the size cap is rotated to daemon.log.1 before the
    // detached child is spawned, instead of being appended to forever (#316).
    let base = temp_home("spawn-log-rotate");
    let config_dir = base.join(".config/moadim");
    std::fs::create_dir_all(&config_dir).unwrap();
    let log_path = config_dir.join("daemon.log");
    std::fs::write(&log_path, vec![b'x'; (DAEMON_LOG_MAX_BYTES + 1) as usize]).unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    let pid = spawn_detached().expect("spawn detached child");
    let rotated = config_dir.join("daemon.log.1");
    assert!(
        rotated.exists(),
        "oversized daemon.log should be rotated to daemon.log.1"
    );
    assert!(
        std::fs::metadata(&log_path).unwrap().len() < DAEMON_LOG_MAX_BYTES,
        "fresh daemon.log should not still hold the oversized contents"
    );
    // The detached child is a real process; kill it so the test doesn't leak one.
    #[cfg(unix)]
    // SAFETY: `pid` is this test's own detached child process; sending it SIGKILL is safe cleanup.
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
    #[cfg(not(unix))]
    let _ = pid;
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn run_background_errors_when_stop_running_times_out() {
    // A server that never stops causes stop_running_and_wait() to time out and
    // return Err, which run_background() propagates (the `?` error branch at L208).
    let server = FakeServer::start(200, String::new());
    let home = temp_home("runbg-stop-err");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "1");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "1");
    assert!(run_background().is_err());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_errors_when_stop_running_times_out() {
    // Same as above but exercises the `?` error branch at L225 (inside `restart()`).
    let server = FakeServer::start(200, String::new());
    let home = temp_home("restart-stop-err");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "1");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "1");
    assert!(restart(false, false).is_err());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_errors_when_spawn_detached_fails() {
    // Server not running → restart() tries spawn_detached() → blocked log dir → Err.
    // Exercises the `?` error branch at L231 (let new_pid = spawn_detached()?).
    let base = temp_home("restart-spawn-err");
    std::fs::create_dir_all(base.join(".config")).unwrap();
    std::fs::write(base.join(".config/moadim"), "block").unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert!(restart(false, false).is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn run_background_errors_when_spawn_detached_fails() {
    // Server not running → run_background() → start_detached_and_report() →
    // spawn_detached() fails → Err propagated.
    // Exercises the `?` error branch at L251 (let pid = spawn_detached()?).
    let base = temp_home("runbg-spawn-err");
    std::fs::create_dir_all(base.join(".config")).unwrap();
    std::fs::write(base.join(".config/moadim"), "block").unwrap();
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", base.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert!(run_background().is_err());
    let _ = std::fs::remove_dir_all(&base);
}
