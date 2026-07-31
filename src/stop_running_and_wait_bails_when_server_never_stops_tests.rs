
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
    kill_pid(424_242);

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

/// Cover the `if let Some(pid) = read_pid_file() { kill_pid(pid); }` closing `}` when
/// `read_pid_file()` returns `None` — i.e. the server is running but no pid file exists.
/// The server then stops on its own (via `stop_after`) so the second `wait_until_stopped()`
/// succeeds and `stop_running_and_wait` returns `Ok`.
#[cfg(unix)]
#[test]
fn stop_running_and_wait_succeeds_without_pid_file_when_server_eventually_stops() {
    let server = FakeServer::start();
    let home = temp_home("no-pid-file");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", &server.addr);
    let _timeout = EnvGuard::set("MOADIM_RESTART_TIMEOUT_MS", "300");
    let _poll = EnvGuard::set("MOADIM_RESTART_POLL_MS", "10");
    // Deliberately write NO pid file: read_pid_file() will return None and the
    // `if let Some(pid)` body is skipped, exercising the closing `}` on that branch.
    //
    // The server stops after the first wait has timed out but before the second wait ends.
    // 450ms (1.5x the 300ms timeout) leaves a wide margin on both sides of the deadline so
    // CPU contention or coverage instrumentation overhead can't flip which window catches the
    // stop (this previously used 60ms/80ms and flaked under `cargo llvm-cov`).
    server.stop_after(Duration::from_millis(450));
    let result = stop_running_and_wait();
    assert!(
        result.is_ok(),
        "should succeed once the server stops: {result:?}"
    );
    let _ = std::fs::remove_dir_all(&home);
}
