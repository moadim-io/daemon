//! Command-line interface: run-mode selection and background-process lifecycle.
//!
//! The `moadim` binary runs an HTTP/MCP/UI server. By default it starts that server **detached in
//! the background** and returns control to the shell — you then manage it from the client (the web
//! UI "STOP" button at the root URL) or with `moadim stop`. Pass `--interactive` to run it in the foreground
//! attached to the terminal instead (Ctrl-C to stop).

use std::io::{Read as _, Write as _};
use std::net::SocketAddr;
use std::time::Duration;

/// Address the server binds to and that the client talks to.
pub const BIND_ADDR: &str = "127.0.0.1:5784";

/// How long to wait when probing or signalling a running server over HTTP.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

/// Environment marker set on the backgrounded child so it knows it was spawned by the launcher.
const DAEMONIZED_ENV: &str = "MOADIM_DAEMONIZED";

/// Process exit code emitted by `status`/`cleanup` when no server is running, so callers can branch
/// on `$?` without parsing stdout. The success case (server reachable) exits `0`.
pub const EXIT_NOT_RUNNING: i32 = 3;

/// Map a server-liveness flag to the script-friendly process exit code: `0` when a server is
/// reachable, [`EXIT_NOT_RUNNING`] when it is not.
fn liveness_exit_code(running: bool) -> i32 {
    if running {
        0
    } else {
        EXIT_NOT_RUNNING
    }
}

/// The action the user asked for on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Run the server in the foreground, attached to the terminal (interactive mode).
    Foreground,
    /// Spawn the server as a detached background process, then exit (the default, non-interactive).
    Background,
    /// Stop a running background server (if any) and start a fresh detached instance.
    Restart,
    /// Ask a running background server to stop. `json` requests machine-readable output.
    Stop {
        /// Emit machine-readable JSON output instead of human-readable text.
        json: bool,
    },
    /// Report whether a server is currently running. `json` requests machine-readable output.
    Status {
        /// Emit machine-readable JSON output instead of human-readable text.
        json: bool,
    },
    /// Ask a running server to reap finished, expired routine run workbenches now. `json` requests
    /// machine-readable output.
    Cleanup {
        /// Emit machine-readable JSON output instead of human-readable text.
        json: bool,
    },
    /// Print usage help.
    Help,
    /// Print the binary version.
    Version,
}

/// Parse CLI arguments (excluding the program name) into a [`Command`].
///
/// Unknown arguments fall back to [`Command::Help`] so the user sees usage rather than a silent
/// no-op. With no arguments the default is [`Command::Background`].
pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    let args: Vec<String> = args.into_iter().collect();
    match args.first().map(String::as_str) {
        None => Command::Background,
        Some("restart") => Command::Restart,
        Some("stop") => Command::Stop {
            json: wants_json(&args[1..]),
        },
        Some("status") => Command::Status {
            json: wants_json(&args[1..]),
        },
        Some("cleanup") => Command::Cleanup {
            json: wants_json(&args[1..]),
        },
        Some("-h" | "--help" | "help") => Command::Help,
        Some("-V" | "--version" | "version") => Command::Version,
        Some("-i" | "--interactive" | "-f" | "--foreground") => Command::Foreground,
        Some("-b" | "--background" | "-d" | "--detach" | "--daemon") => Command::Background,
        Some(_) => Command::Help,
    }
}

/// Whether a `--json` flag appears among a command's trailing arguments, requesting
/// machine-readable output for `status`/`cleanup`.
fn wants_json(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--json")
}

/// Print usage help to stdout.
pub fn print_help() {
    println!(
        "moadim — cron/MCP/REST server with a web control panel\n\
         \n\
         USAGE:\n\
         \x20   moadim [MODE]\n\
         \x20   moadim <COMMAND>\n\
         \n\
         MODES:\n\
         \x20   (default)              start the server in the background and exit\n\
         \x20   -i, --interactive      run in the foreground, attached to the terminal (Ctrl-C to stop)\n\
         \x20   -b, --background       start the server detached in the background (explicit default)\n\
         \n\
         COMMANDS:\n\
         \x20   restart                stop a running server (if any) and start a fresh background one\n\
         \x20   stop [--json]          stop a running background server\n\
         \x20   status [--json]        show whether a server is running\n\
         \x20   cleanup [--json]       reap finished, expired routine workbenches now\n\
         \x20   help, -h, --help       show this help\n\
         \x20   version, -V            show the version\n\
         \n\
         Pass --json to `stop`/`status`/`cleanup` for a single-line machine-readable object.\n\
         `status`/`cleanup`/`stop` exit 0 when a server is running and 3 when none is, so scripts\n\
         can branch on $? without parsing stdout.\n\
         \n\
         Once running, manage the server from the web client at http://{BIND_ADDR}\n\
         (the STOP button) or with `moadim stop`."
    );
}

/// Print the binary version to stdout.
pub fn print_version() {
    println!("moadim {}", env!("CARGO_PKG_VERSION"));
}

/// Start the server as a detached background process and return immediately.
///
/// If a server is already responding on [`BIND_ADDR`], it is stopped and replaced with a fresh
/// process so each launch yields a clean instance.
pub fn run_background() -> anyhow::Result<()> {
    if is_running() {
        let pid = read_pid_file()
            .map(|process_id| format!(" (pid {process_id})"))
            .unwrap_or_default();
        println!("moadim is already running{pid}; stopping it to start a fresh instance");
        crate::restart::stop_running_and_wait()?;
    }
    start_detached_and_report("started")
}

/// Stop a running background server (if any) and start a fresh detached instance.
///
/// Unlike [`run_background`], which restarts only as a side effect of being asked to start while
/// one is already up, this is the explicit "give me a clean process now" command: it stops the
/// running server when present, otherwise just starts one.
pub fn restart() -> anyhow::Result<()> {
    let old_pid = if is_running() {
        let pid = read_pid_file();
        let suffix = pid
            .map(|process_id| format!(" (pid {process_id})"))
            .unwrap_or_default();
        println!("moadim is running{suffix}; stopping it");
        crate::restart::stop_running_and_wait()?;
        pid
    } else {
        println!("moadim is not running; starting a fresh instance");
        None
    };
    let new_pid = spawn_detached()?;
    // Headline the rotation so scripts/logs can see the process actually changed.
    println!("{}", restart_rotation_line(old_pid, new_pid));
    report_endpoints();
    Ok(())
}

/// Format the one-line PID rotation summary `restart` prints, e.g. `restarted: pid 123 -> 456`.
///
/// `old` is the PID of the server that was stopped; when nothing was running (or its PID could not
/// be read) the old side reads `none`, e.g. `restarted: pid none -> 456`.
fn restart_rotation_line(old: Option<u32>, new: u32) -> String {
    let old = old.map_or_else(|| "none".to_string(), |pid| pid.to_string());
    format!("restarted: pid {old} -> {new}")
}

/// Spawn a detached server process and print where to reach and manage it.
///
/// `verb` describes how the process came to be ("started" / "restarted") for the first line.
fn start_detached_and_report(verb: &str) -> anyhow::Result<()> {
    let pid = spawn_detached()?;
    println!("moadim {verb} in the background (pid {pid}) at http://{BIND_ADDR}");
    report_endpoints();
    Ok(())
}

/// Print the reach/manage hints (UI, stop, logs) shared by every detached-launch report.
fn report_endpoints() {
    println!("  UI    http://{BIND_ADDR}");
    println!("  stop  moadim stop   (or use the STOP button in the UI)");
    println!("  logs  {}", paths_daemon_log());
}

/// Ask a running server to stop via the `/shutdown` route. With `json`, emits a single
/// machine-readable object (`{"running":bool,"pid":N|null,"address":…}`, matching `status --json`'s
/// shape) instead of the human-readable line.
///
/// Returns the process exit code to surface, mirroring the `status`/`cleanup` contract: `0` when a
/// running server was asked to shut down, and [`EXIT_NOT_RUNNING`] when none was reachable, so
/// scripts can branch on `$?` without parsing stdout.
pub fn stop(json: bool) -> anyhow::Result<i32> {
    // Read the PID before asking the server to stop: a graceful shutdown clears the pid file, so
    // the only reliable moment to capture which process we stopped is *before* the request.
    let pid = read_pid_file();
    match http_request("POST", "/api/v1/shutdown") {
        Ok(200) => {
            if json {
                println!("{}", stop_json(true, pid));
            } else {
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
            } else {
                println!("moadim is not running");
            }
            Ok(liveness_exit_code(false))
        }
    }
}

/// Render the `stop` result as a one-line JSON object:
/// `{"running":bool,"pid":N|null,"address":…}`, matching `status --json`'s shape exactly so both
/// can be parsed uniformly. `running` is `true` when a running server was asked to shut down, and
/// `false` when none was reachable. `pid` is the process that was stopped (read from the pid file
/// before the shutdown request), or `null` when no pid file was present. `address` is the bound
/// [`BIND_ADDR`] the request was sent to.
fn stop_json(running: bool, pid: Option<u32>) -> String {
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": BIND_ADDR,
    })
    .to_string()
}

/// Ask a running server to reap finished, expired routine run workbenches now, and print the count.
///
/// Runs the same sweep as the hourly background task instead of waiting for the next tick, via the
/// `/api/v1/routines/cleanup` route. Prints how many workbenches were removed, or a hint when no
/// server is up. With `json`, emits a single machine-readable object instead so the result can be
/// piped into scripts.
///
/// Returns the process exit code to surface: `0` when the server handled the sweep, and
/// [`EXIT_NOT_RUNNING`] when no server is running, so scripts can branch on `$?`.
pub fn cleanup(json: bool) -> anyhow::Result<i32> {
    match http_request_with_body("POST", "/api/v1/routines/cleanup") {
        Ok((200, body)) => {
            let removed = parse_removed_count(&body).unwrap_or(0);
            if json {
                println!("{}", cleanup_json(removed, true));
            } else {
                let plural = if removed == 1 { "" } else { "es" };
                println!("cleanup removed {removed} workbench{plural}");
            }
            Ok(liveness_exit_code(true))
        }
        Ok((status, _)) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            if json {
                println!("{}", cleanup_json(0, false));
            } else {
                println!("moadim is not running");
            }
            Ok(liveness_exit_code(false))
        }
    }
}

/// Report whether a server is running, with its PID when known. With `json`, emits a single
/// machine-readable object instead of the human-readable line.
///
/// Returns the process exit code to surface: `0` when a server is reachable, and
/// [`EXIT_NOT_RUNNING`] when not, so scripts can branch on `$?` without parsing stdout.
pub fn status(json: bool) -> anyhow::Result<i32> {
    let running = is_running();
    let pid = read_pid_file();
    if json {
        println!("{}", status_json(running, pid));
        return Ok(liveness_exit_code(running));
    }
    if running {
        let pid_suffix = pid
            .map(|process_id| format!(" (pid {process_id})"))
            .unwrap_or_default();
        println!("moadim is running{pid_suffix} at http://{BIND_ADDR}");
    } else {
        println!("moadim is not running");
    }
    Ok(liveness_exit_code(running))
}

/// Render the `status` result as a one-line JSON object: `{"running":bool,"pid":N|null,"address":…}`.
/// `pid` is `null` when no pid file is present (or the server is down).
fn status_json(running: bool, pid: Option<u32>) -> String {
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": BIND_ADDR,
    })
    .to_string()
}

/// Render the `cleanup` result as a one-line JSON object: `{"running":bool,"removed":N}`. `removed`
/// is `0` when the server is not running (`running:false`).
fn cleanup_json(removed: usize, running: bool) -> String {
    serde_json::json!({
        "running": running,
        "removed": removed,
    })
    .to_string()
}

/// Write the current process PID into the pid file so `stop`/`status` and signals can find it.
pub fn write_pid_file() -> anyhow::Result<()> {
    let path = crate::paths::pid_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    ensure_config_gitignore();
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

/// Write a `.gitignore` into the config dir so generated runtime files (`*.pid`, `*.log`)
/// stay out of version control when users track `~/.config/moadim` in a dotfiles repo.
/// Best-effort: failure to write it is not fatal to starting the daemon.
fn ensure_config_gitignore() {
    let gitignore = crate::paths::config_gitignore_path();
    if !gitignore.exists() {
        let _ = std::fs::write(&gitignore, "*.pid\n*.log\n");
    }
}

/// Remove the pid file. Best-effort: a missing file is not an error.
pub fn clear_pid_file() {
    let _ = std::fs::remove_file(crate::paths::pid_file());
}

/// Read the PID recorded in the pid file, if present and parseable.
pub(crate) fn read_pid_file() -> Option<u32> {
    std::fs::read_to_string(crate::paths::pid_file())
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The daemon log path, rendered for display.
fn paths_daemon_log() -> String {
    crate::paths::daemon_log_file().display().to_string()
}

/// Returns `true` if a server answers `GET /health` on [`BIND_ADDR`].
pub(crate) fn is_running() -> bool {
    matches!(http_request("GET", "/api/v1/health"), Ok(200))
}

/// Send a minimal HTTP/1.1 request to the local server and return the response status code.
pub(crate) fn http_request(method: &str, path: &str) -> std::io::Result<u16> {
    http_request_with_body(method, path).map(|(status, _)| status)
}

/// Send a minimal HTTP/1.1 request and return the response status code together with its body.
fn http_request_with_body(method: &str, path: &str) -> std::io::Result<(u16, String)> {
    let addr: SocketAddr = BIND_ADDR
        .parse()
        .expect("BIND_ADDR is a valid socket address");
    let mut stream = std::net::TcpStream::connect_timeout(&addr, PROBE_TIMEOUT)?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT))?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT))?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {BIND_ADDR}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
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
fn parse_status_code(resp: &str) -> Option<u16> {
    resp.lines().next()?.split_whitespace().nth(1)?.parse().ok()
}

/// Return the body of a raw HTTP response — everything after the blank line that ends the headers.
fn parse_body(resp: &str) -> String {
    resp.split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default()
}

/// Extract the `removed` count from a [`CleanupResponse`](crate::routines::CleanupResponse) JSON
/// body (`{"removed": N}`).
fn parse_removed_count(body: &str) -> Option<usize> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("removed")?.as_u64().map(|n| n as usize)
}

/// Spawn a detached copy of this binary running the server in the foreground, returning its PID.
///
/// The child runs with `--interactive` (so it actually serves), in its own process group so a
/// terminal SIGINT to the launcher does not reach it, with stdio redirected to the daemon log.
fn spawn_detached() -> anyhow::Result<u32> {
    use std::process::{Command as Proc, Stdio};

    let exe = std::env::current_exe()?;
    let log_path = crate::paths::daemon_log_file();
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err = out.try_clone()?;

    let mut cmd = Proc::new(exe);
    cmd.arg("--interactive")
        .env(DAEMONIZED_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    detach(&mut cmd);

    let child = cmd.spawn()?;
    Ok(child.id())
}

/// Put the spawned child in its own process group so it survives the launcher and terminal signals.
#[cfg(unix)]
fn detach(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
}

/// No-op on platforms without process groups; the child still detaches via redirected stdio.
#[cfg(not(unix))]
fn detach(_cmd: &mut std::process::Command) {}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;
