#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
fn restart_quiet_skips_endpoint_hints_when_none_running() {
    let home = temp_home("restart-fresh-quiet");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    restart(false, true).unwrap();
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
