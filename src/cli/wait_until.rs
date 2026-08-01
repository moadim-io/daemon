#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Call `check` repeatedly (sleeping `WAIT_POLL_INTERVAL` between attempts) until it returns
/// `true` or `timeout` elapses. Returns whether it ever returned `true`. Always calls `check` at
/// least once, even when `timeout` is zero.
pub(crate) fn wait_until(mut check: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if check() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(WAIT_POLL_INTERVAL);
    }
}

/// Send a minimal HTTP/1.1 request to the local server and return the response status code.
pub(crate) fn http_request(method: &str, path: &str) -> std::io::Result<u16> {
    http_request_with_body(method, path).map(|(status, _)| status)
}

/// How long to wait on a data-plane request (`create`/`trigger`/etc.). More generous than
/// [`PROBE_TIMEOUT`] because these routes can do real work (crontab sync, workbench spawn) before
/// responding, whereas a liveness probe only needs the server to answer `GET /health` promptly.
pub(crate) const DATA_OP_TIMEOUT: Duration = Duration::from_secs(10);

/// Send a minimal HTTP/1.1 request (no body) and return the response status code with its body.
pub(crate) fn http_request_with_body(method: &str, path: &str) -> std::io::Result<(u16, String)> {
    http_request_core(method, path, None, PROBE_TIMEOUT)
}

/// Send a minimal HTTP/1.1 request with an optional JSON `body` and return the response status code
/// together with its body, using the generous [`DATA_OP_TIMEOUT`]. Data-plane CLI subcommands
/// ([`crate::commands`]) use this to drive the running server's `/api/v1` routes over the same
/// loopback client the lifecycle commands use.
pub(crate) fn http_request_json(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> std::io::Result<(u16, String)> {
    http_request_core(method, path, body, DATA_OP_TIMEOUT)
}

/// Core minimal HTTP/1.1 client: connect to the local server, send `method path` with an optional
/// JSON `body`, and return the response status code together with its body. `timeout` bounds the
/// connect/read/write so a hung or absent server fails fast.
pub(crate) fn http_request_core(
    method: &str,
    path: &str,
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<(u16, String)> {
    let addr_str = crate::cli::bind_addr();
    let addr: SocketAddr = addr_str.parse().map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid bind address {addr_str:?}: {err}"),
        )
    })?;
    let mut stream = std::net::TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let payload = body.unwrap_or_default();
    let auth_header = crate::cli::api_token()
        .map(|token| format!("Authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr_str}\r\n{auth_header}Content-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    // Unlike the read below, a failed write here means the request never went out at all, so
    // there is no partial response to salvage — propagate the error via `?` like the connect
    // above, instead of panicking. The server can legitimately close the connection between
    // `connect_timeout` succeeding and this write running (e.g. mid-`restart`, while the old
    // server is being killed), and every caller already matches on this function's `Result` to
    // degrade gracefully ("moadim is not running") rather than crash with a panic trace.
    stream.write_all(req.as_bytes())?;
    let mut resp = String::new();
    // A failed read after a clean shutdown can still yield the status line we already received.
    let _ = stream.read_to_string(&mut resp);
    let status = parse_status_code(&resp).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no HTTP status line in response",
        )
    })?;
    Ok((status, parse_body(&resp)))
}

/// Extract the numeric status code from an HTTP response's status line (e.g. `HTTP/1.1 200 OK`).
pub(crate) fn parse_status_code(resp: &str) -> Option<u16> {
    resp.lines().next()?.split_whitespace().nth(1)?.parse().ok()
}

/// Return the body of a raw HTTP response — everything after the blank line that ends the headers.
pub(crate) fn parse_body(resp: &str) -> String {
    resp.split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default()
}

/// Extract the `removed` count from a [`CleanupResponse`](crate::routines::CleanupResponse) JSON
/// body (`{"removed": N}`).
pub(crate) fn parse_removed_count(body: &str) -> Option<usize> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("removed")?.as_u64().map(|n| n as usize)
}

/// Extract the `freed_bytes` total from a [`CleanupResponse`](crate::routines::CleanupResponse) JSON
/// body. Returns `None` for a body lacking the (additive) field, so older servers degrade to `0`.
pub(crate) fn parse_freed_bytes(body: &str) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("freed_bytes")?.as_u64()
}

/// Spawn a detached copy of this binary running the server in the foreground, returning its PID.
///
/// The child runs with `--interactive` (so it actually serves), in its own process group so a
/// terminal SIGINT to the launcher does not reach it, with stdio redirected to the daemon log.
pub(crate) fn spawn_detached() -> anyhow::Result<u32> {
    spawn_detached_with(|cmd| {
        cmd.arg("--interactive")
            .env(crate::cli::DAEMONIZED_ENV, "1");
    })
}

/// Spawn a detached helper that stops the currently-running server and starts a fresh one,
/// returning the helper's PID. Used by the `/api/v1/restart` route and the `restart` MCP tool so the
/// daemon can be cycled from any surface, not just the CLI: the in-process server cannot rebind its
/// own port, so it delegates the stop-old-then-start-new dance to this separate process.
///
/// The helper is launched with the `--background` flag rather than the `restart` subcommand on
/// purpose: `moadim --background` ([`crate::cli::run_background`]) already stops a running instance before
/// starting a fresh one, and passing a flag (not a bare positional) means that under the test
/// harness — where `current_exe` is the test binary — the child is rejected immediately instead of
/// being interpreted as a test-name filter that would re-enter these very tests.
pub fn spawn_restart() -> anyhow::Result<u32> {
    spawn_detached_with(|cmd| {
        cmd.arg("--background");
    })
}

/// Spawn a detached copy of this binary with stdio redirected to the daemon log and its own process
/// group, applying `configure` to set the subcommand/flags before launch. Returns the child PID.
pub(crate) fn spawn_detached_with(
    configure: impl FnOnce(&mut std::process::Command),
) -> anyhow::Result<u32> {
    use std::process::{Command as Proc, Stdio};

    let exe = crate::utils::process::current_exe()
        .map_err(|err| anyhow::anyhow!("resolve current executable path: {err}"))?;
    let log_path = crate::paths::daemon_log_file();
    let log_parent = crate::utils::fs_perms::parent_or_err(&log_path, "daemon log")?;
    crate::utils::fs_perms::create_private_dir_all(log_parent)?;
    rotate_daemon_log_if_due(&log_path);
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err = out.try_clone()?;

    let mut cmd = Proc::new(exe);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    configure(&mut cmd);
    detach(&mut cmd);

    #[allow(
        clippy::zombie_processes,
        reason = "intentionally detached: the child outlives this process and is reaped by the OS/service manager, not waited on here"
    )]
    let child = cmd.spawn()?;
    Ok(child.id())
}

/// Put the spawned child in its own process group so it survives the launcher and terminal signals.
#[cfg(unix)]
pub(crate) fn detach(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
}

/// No-op on platforms without process groups; the child still detaches via redirected stdio.
#[cfg(not(unix))]
pub(crate) fn detach(_cmd: &mut std::process::Command) {}
