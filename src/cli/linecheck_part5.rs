
/// Refuse an interactive foreground start (`moadim -i`) when a server is already reachable on the
/// bind address, instead of letting the later bind fail with an opaque OS error
/// (`Address already in use (os error 48)`) that gives no hint a real daemon is already up.
///
/// Unlike [`run_background`], which silently stops and replaces a running instance, an interactive
/// run *refuses* and points at `moadim stop` / `moadim restart`: attaching a second foreground
/// process to the terminal is rarely what the user intended, and silently killing the existing one
/// would be a surprising side effect of `-i`.
///
/// The launcher-spawned background child also runs with `--interactive`, but it *is* the freshly
/// started server (the launcher already stopped any prior instance), so the preflight is skipped for
/// it via the [`DAEMONIZED_ENV`] marker.
pub fn ensure_not_running_for_foreground() -> anyhow::Result<()> {
    if std::env::var_os(DAEMONIZED_ENV).is_some() {
        return Ok(());
    }
    foreground_preflight(is_running(), read_pid_file())
}

/// Decide the foreground-start preflight outcome from whether a server is already reachable and its
/// pid: `Ok(())` to proceed with the bind, or an error carrying user-facing guidance.
///
/// Split from [`ensure_not_running_for_foreground`] so both outcomes are unit-testable without a
/// live network probe.
pub(crate) fn foreground_preflight(running: bool, pid: Option<u32>) -> anyhow::Result<()> {
    if running {
        anyhow::bail!("{}", foreground_already_running_message(pid));
    }
    Ok(())
}

/// User-facing message when an interactive start is refused: names the running pid when known and
/// points at the commands that resolve it.
pub(crate) fn foreground_already_running_message(pid: Option<u32>) -> String {
    let suffix = pid
        .map(|process_id| format!(" (pid {process_id})"))
        .unwrap_or_default();
    format!(
        "moadim is already running{suffix}; refusing to start a second foreground instance. \
         Stop it with `moadim stop`, or replace it with `moadim restart`."
    )
}

/// Ask a running server to stop via the `/shutdown` route. With `json`, emits a single
/// machine-readable object (`{"running":bool,"pid":N|null,"address":…}`, matching `status --json`'s
/// shape) instead of the human-readable line. With `quiet`, the human-readable line is suppressed
/// entirely (ignored under `json`), so scripts that branch on `$?` alone get no stdout noise.
///
/// Returns the process exit code to surface, mirroring the `status`/`cleanup` contract: `0` when a
/// running server was asked to shut down, and [`EXIT_NOT_RUNNING`] when none was reachable, so
/// scripts can branch on `$?` without parsing stdout.
///
/// This only stops the daemon's HTTP/MCP server; a routine agent already running in a detached
/// tmux session (started via `tmux new-session -d`) is independent of the daemon process and is
/// **not** killed by this call. It keeps running — and can keep opening PRs, filing issues, etc. —
/// until it finishes on its own or a future daemon start's watchdog/cleanup sweep reaps it
/// (issue #320).
pub fn stop(json: bool, quiet: bool) -> anyhow::Result<i32> {
    // Read the PID before asking the server to stop: a graceful shutdown clears the pid file, so
    // the only reliable moment to capture which process we stopped is *before* the request.
    let pid = read_pid_file();
    match http_request("POST", "/api/v1/shutdown") {
        Ok(200) => {
            if json {
                println!("{}", stop_json(true, pid));
            } else if !quiet {
                println!("moadim is shutting down");
            }
            Ok(liveness_exit_code(true))
        }
        Ok(status) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            if json {
                println!("{}", stop_json(false, pid));
            } else if !quiet {
                println!("moadim is not running");
            }
            Ok(liveness_exit_code(false))
        }
    }
}
