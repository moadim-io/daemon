#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
    assert_eq!(trigger("some-id").unwrap(), 0);
}

#[test]
fn trigger_reports_unknown_routine_on_404() {
    // A 404 from the trigger route means no routine has that id — a user error, surfaced as a
    // non-zero exit via the bubbled `Err`, distinct from "server not running".
    let server = FakeServer::start(404, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(trigger("missing").is_err());
}

#[test]
fn trigger_errors_on_unexpected_status() {
    let server = FakeServer::start(500, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(trigger("some-id").is_err());
}

#[test]
fn trigger_reports_not_running_when_no_server() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(trigger("some-id").unwrap(), EXIT_NOT_RUNNING);
}

#[test]
fn logs_prints_the_response_body_when_server_responds() {
    let server = FakeServer::start(200, "line one\nline two\n".to_string());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(logs("some-id").unwrap(), 0);
}

#[test]
fn logs_succeeds_on_an_empty_body() {
    // No run yet: `svc_logs` returns an empty string, which is a normal, successful outcome.
    let server = FakeServer::start(200, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(logs("some-id").unwrap(), 0);
}

#[test]
fn logs_reports_unknown_routine_on_404() {
    let server = FakeServer::start(404, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(logs("missing").is_err());
}

#[test]
fn logs_errors_on_unexpected_status() {
    let server = FakeServer::start(500, String::new());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert!(logs("some-id").is_err());
}

#[test]
fn logs_reports_not_running_when_no_server() {
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(logs("some-id").unwrap(), EXIT_NOT_RUNNING);
}
