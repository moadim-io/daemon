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

/// Environment variable overriding [`BIND_ADDR`] (test seam): lets tests run the server and probe
/// it on an ephemeral port instead of the fixed default, so they never collide with a real daemon.
const BIND_ADDR_ENV: &str = "MOADIM_BIND_ADDR";

/// The socket address to bind/probe, honoring the [`BIND_ADDR_ENV`] override when set.
pub fn bind_addr() -> String {
    std::env::var(BIND_ADDR_ENV).unwrap_or_else(|_| BIND_ADDR.to_string())
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        /// Suppress the human-readable status line so scripts that branch on `$?` get no stdout
        /// noise. Ignored under `json`, which always prints its single object.
        quiet: bool,
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
    /// Trigger a routine to run immediately, outside its schedule, by UUID.
    Trigger {
        /// UUID of the routine to trigger.
        id: String,
    },
    /// Register the daemon as an OS service (launchd on macOS, systemd user on Linux).
    Install,
    /// Remove the OS service registration created by [`Command::Install`].
    Uninstall,
    /// Print usage help.
    Help,
    /// Print the binary version.
    Version,
    /// A data-plane subcommand (`routines`, `agents`, `echo`) handled by the clap-based
    /// [`crate::commands`] dispatcher, which talks to the running server over HTTP. Carries the raw
    /// argv (including the subcommand keyword) for clap to parse.
    Data(Vec<String>),
    /// A `machine` subcommand (`show`/`set`/`list`) handled locally by [`crate::machine`] — it reads
    /// or writes this install's machine identity without a running server. Carries the args *after*
    /// the `machine` keyword.
    Machine(Vec<String>),
}

/// First-argument keywords that select a data-plane subcommand handled by [`crate::commands`]
/// rather than the lifecycle commands parsed here. Kept in sync with the clap subcommands.
pub(crate) const DATA_COMMANDS: &[&str] = &["routines", "schedule", "agents", "echo"];

/// Parse CLI arguments (excluding the program name) into a [`Command`].
///
/// Unknown arguments fall back to [`Command::Help`] so the user sees usage rather than a silent
/// no-op. With no arguments the default is [`Command::Background`].
pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    let args: Vec<String> = args.into_iter().collect();
    match args.first().map(String::as_str) {
        None => Command::Background,
        Some(first) if DATA_COMMANDS.contains(&first) => Command::Data(args),
        Some("machine") => Command::Machine(args[1..].to_vec()),
        Some("restart") => Command::Restart,
        Some("stop") => Command::Stop {
            json: wants_json(&args[1..]),
            quiet: wants_quiet(&args[1..]),
        },
        Some("status") => Command::Status {
            json: wants_json(&args[1..]),
        },
        Some("cleanup") => Command::Cleanup {
            json: wants_json(&args[1..]),
        },
        // `trigger <id>` runs a single routine on demand. Without an id there is nothing to
        // trigger, so fall back to help rather than silently no-op (mirrors the unknown-argument
        // behavior). `run` is kept as a hidden back-compat alias of the original subcommand name.
        Some("trigger" | "run") => match args.get(1) {
            Some(id) => Command::Trigger { id: id.clone() },
            None => Command::Help,
        },
        Some("install") => Command::Install,
        Some("uninstall") => Command::Uninstall,
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

/// Whether a `--quiet`/`-q` flag appears among a command's trailing arguments, requesting that
/// `stop` suppress its human-readable status line.
fn wants_quiet(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--quiet" || arg == "-q")
}

/// Print usage help to stdout.
pub fn print_help() {
    let bind_addr = bind_addr();
    println!(
        "moadim — routine scheduler with an MCP/REST API and a web control panel\n\
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
         \x20   stop [--json] [-q]     stop a running background server (-q/--quiet: no stdout)\n\
         \x20   status [--json]        show whether a server is running\n\
         \x20   cleanup [--json]       reap finished, expired routine workbenches now\n\
         \x20   trigger <id>           trigger a routine to run now, outside its schedule\n\
         \x20   install                register moadim as an OS service (launchd / systemd user)\n\
         \x20   uninstall              remove the OS service registration and the managed crontab block\n\
         \x20   machine <show|set|list> show/set this machine's identity, or list machines referenced\n\
         \x20   help, -h, --help       show this help\n\
         \x20   version, -V            show the version\n\
         \n\
         DATA COMMANDS (talk to the running server over HTTP; pass --help for flags):\n\
         \x20   routines  <create|list|get|update|replace|delete|trigger|logs|ical> ...\n\
         \x20   schedule  trigger <id> trigger a routine by ID (used by the routines crontab line)\n\
         \x20   agents                 list available agent keys\n\
         \x20   echo <message>         echo a message via the server\n\
         \n\
         Pass --json to `stop`/`status`/`cleanup` for a single-line machine-readable object.\n\
         `status`/`cleanup`/`stop` exit 0 when a server is running and 3 when none is, so scripts\n\
         can branch on $? without parsing stdout.\n\
         \n\
         Once running, manage the server from the web client at http://{bind_addr}\n\
         (the STOP button) or with `moadim stop`."
    );
}

/// Print the binary version to stdout, including the git commit and date it was
/// built from when available (e.g. `moadim 0.1.0 (a1b2c3d 2026-06-19)`).
pub fn print_version() {
    println!("moadim {}", crate::build_info::long_version());
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
fn foreground_preflight(running: bool, pid: Option<u32>) -> anyhow::Result<()> {
    if running {
        anyhow::bail!("{}", foreground_already_running_message(pid));
    }
    Ok(())
}

/// User-facing message when an interactive start is refused: names the running pid when known and
/// points at the commands that resolve it.
fn foreground_already_running_message(pid: Option<u32>) -> String {
    let suffix = pid
        .map(|process_id| format!(" (pid {process_id})"))
        .unwrap_or_default();
    format!(
        "moadim is already running{suffix}; refusing to start a second foreground instance. \
         Stop it with `moadim stop`, or replace it with `moadim restart`."
    )
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
    println!(
        "moadim {verb} in the background (pid {pid}) at http://{}",
        bind_addr()
    );
    report_endpoints();
    Ok(())
}

/// Print the reach/manage hints (UI, stop, logs) shared by every detached-launch report.
fn report_endpoints() {
    println!("  UI    http://{}", bind_addr());
    println!("  stop  moadim stop   (or use the STOP button in the UI)");
    println!("  logs  {}", paths_daemon_log());
}

/// Ask a running server to stop via the `/shutdown` route. With `json`, emits a single
/// machine-readable object (`{"running":bool,"pid":N|null,"address":…}`, matching `status --json`'s
/// shape) instead of the human-readable line. With `quiet`, the human-readable line is suppressed
/// entirely (ignored under `json`), so scripts that branch on `$?` alone get no stdout noise.
///
/// Returns the process exit code to surface, mirroring the `status`/`cleanup` contract: `0` when a
/// running server was asked to shut down, and [`EXIT_NOT_RUNNING`] when none was reachable, so
/// scripts can branch on `$?` without parsing stdout.
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

/// Render the `stop` result as a one-line JSON object:
/// `{"running":bool,"pid":N|null,"address":…}`, matching `status --json`'s shape exactly so both
/// can be parsed uniformly. `running` is `true` when a running server was asked to shut down, and
/// `false` when none was reachable. `pid` is the process that was stopped (read from the pid file
/// before the shutdown request), or `null` when no pid file was present. `address` is the bound
/// address the request was sent to ([`bind_addr`], honoring the `MOADIM_BIND_ADDR` override) so it
/// stays identical to `status --json` under a non-default bind.
fn stop_json(running: bool, pid: Option<u32>) -> String {
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": bind_addr(),
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

/// Ask a running server to trigger routine `id` immediately, outside its schedule, via the
/// `POST /routines/{id}/trigger` route — the same on-demand run the REST API and MCP tool already
/// expose, finally reachable from the terminal.
///
/// Prints a confirmation when the routine was triggered, an error when no routine has that id
/// (`404`), and a "not running" hint when no server is reachable. Returns the process exit code to
/// surface, mirroring the `status`/`cleanup` contract: `0` when the routine was triggered, and
/// [`EXIT_NOT_RUNNING`] when no server is running, so scripts can branch on `$?`.
pub fn trigger(id: String) -> anyhow::Result<i32> {
    match http_request("POST", &format!("/api/v1/routines/{id}/trigger")) {
        Ok(200) => {
            println!("triggered routine {id}");
            Ok(liveness_exit_code(true))
        }
        Ok(404) => {
            anyhow::bail!("no routine with id {id}");
        }
        Ok(status) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            println!("moadim is not running");
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
        // Fold the server's own /health (uptime + version) into the object so a single
        // `status --json` answers liveness *and* age/version without a second call. When the
        // server is down (or answers unparseably) these fields are emitted as null.
        let health = if running { fetch_health() } else { None };
        println!("{}", status_json(running, pid, health));
        return Ok(liveness_exit_code(running));
    }
    if running {
        let pid_suffix = pid
            .map(|process_id| format!(" (pid {process_id})"))
            .unwrap_or_default();
        println!("moadim is running{pid_suffix} at http://{}", bind_addr());
    } else {
        println!("moadim is not running");
    }
    Ok(liveness_exit_code(running))
}

/// Server-sourced liveness details pulled from `GET /health` to enrich `status --json`.
#[derive(Debug, PartialEq, Eq)]
struct HealthInfo {
    /// Seconds the server reports it has been up.
    uptime_secs: u64,
    /// The daemon version the server reports.
    version: String,
}

/// Render the `status` result as a one-line JSON object:
/// `{"running":bool,"pid":N|null,"address":…,"uptime_secs":N|null,"version":S|null}`.
///
/// `pid` is `null` when no pid file is present (or the server is down). `uptime_secs`/`version`
/// carry the running server's self-reported `/health` details (via `health`), and are `null` when
/// no server answers or its `/health` body could not be parsed.
fn status_json(running: bool, pid: Option<u32>, health: Option<HealthInfo>) -> String {
    let uptime_secs = health.as_ref().map(|info| info.uptime_secs);
    let version = health.as_ref().map(|info| info.version.as_str());
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": bind_addr(),
        "uptime_secs": uptime_secs,
        "version": version,
    })
    .to_string()
}

/// Probe the running server's `GET /health` and return its uptime/version, or `None` when the
/// request fails, the status is not `200`, or the body is not the expected JSON shape.
fn fetch_health() -> Option<HealthInfo> {
    let (status, body) = http_request_with_body("GET", "/api/v1/health").ok()?;
    (status == 200).then(|| parse_health(&body)).flatten()
}

/// Extract `uptime_secs` and `version` from a [`HealthResponse`](crate::routes::http::HealthResponse)
/// JSON body. Returns `None` if either field is missing or the wrong type.
fn parse_health(body: &str) -> Option<HealthInfo> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let uptime_secs = value.get("uptime_secs")?.as_u64()?;
    let version = value.get("version")?.as_str()?.to_string();
    Some(HealthInfo {
        uptime_secs,
        version,
    })
}

/// Render the `cleanup` result as a one-line JSON object:
/// `{"running":bool,"removed":N,"address":…}`. `removed` is `0` when the server is not running
/// (`running:false`). `address` is the effective bound [`bind_addr`] the request was sent to,
/// matching `status --json`/`stop --json`'s object shape so every `--json` command surfaces the
/// endpoint it talked to.
fn cleanup_json(removed: usize, running: bool) -> String {
    serde_json::json!({
        "running": running,
        "removed": removed,
        "address": bind_addr(),
    })
    .to_string()
}

/// Write the current process PID into the pid file so `stop`/`status` and signals can find it.
pub fn write_pid_file() -> anyhow::Result<()> {
    let path = crate::paths::pid_file();
    std::fs::create_dir_all(path.parent().expect("pid file path has a parent dir"))?;
    ensure_config_gitignore();
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

/// Ensure the config dir `.gitignore` contains all required patterns on every start.
///
/// Reads the existing file (if any), appends any missing patterns, and writes back only when
/// something changed. Preserves user-added entries. Best-effort: failure is not fatal.
fn ensure_config_gitignore() {
    const REQUIRED: &[&str] = &["*.pid", "*.log", "*.local.*"];
    let gitignore = crate::paths::config_gitignore_path();
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    let lines: Vec<&str> = existing.lines().collect();
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|pat| !lines.iter().any(|line| line.trim() == *pat))
        .collect();
    if missing.is_empty() {
        return;
    }
    let mut content = existing.clone();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for pattern in &missing {
        content.push_str(pattern);
        content.push('\n');
    }
    let _ = std::fs::write(&gitignore, &content);
}

/// Remove the pid file. Best-effort: a missing file is not an error.
pub fn clear_pid_file() {
    let _ = std::fs::remove_file(crate::paths::pid_file());
}

/// Read the PID recorded in the pid file, if present, parseable, **and still a live process**.
///
/// On a clean shutdown the pid file is removed ([`clear_pid_file`]), but a `kill -9`, panic, OOM
/// kill, or power loss skips that path and leaves the file behind with a now-dead PID. Returning
/// that stale PID would make the machine-readable contract dishonest — `status`/`stop --json` would
/// report a `pid` for a process that no longer exists (and which, after PID reuse, may belong to an
/// unrelated process), and `restart` would force-kill it. So a recorded PID that is not alive is
/// treated as absent and the stale file is cleaned up best-effort, keeping the pid self-healing.
pub(crate) fn read_pid_file() -> Option<u32> {
    let pid = std::fs::read_to_string(crate::paths::pid_file())
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if process_is_alive(pid) {
        Some(pid)
    } else {
        clear_pid_file();
        None
    }
}

/// Returns `true` if a process with `pid` currently exists.
///
/// Uses the standard signal-0 liveness probe (`kill -0 <pid>`): no signal is delivered, but the
/// kernel runs the same existence check it would for a real signal, so a successful exit means the
/// PID is alive. Unlike [`crate::restart`]'s destructive killer, this probe is harmless, so it goes
/// straight to the real `kill` with no env override to keep parallel tests from racing on it.
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    // Linux PIDs are signed; u32::MAX overflows to -1 in the kernel, causing kill(-1, 0)
    // to return success (it sends to all accessible processes). Treat out-of-range as dead.
    if pid > i32::MAX as u32 {
        return false;
    }
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Best-effort liveness on non-unix: assume the recorded PID is alive, mirroring the best-effort
/// semantics of the Windows force-killer. The pid file is still cleared on graceful shutdown.
#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
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

/// How long to wait on a data-plane request (`create`/`trigger`/etc.). More generous than
/// [`PROBE_TIMEOUT`] because these routes can do real work (crontab sync, workbench spawn) before
/// responding, whereas a liveness probe only needs the server to answer `GET /health` promptly.
const DATA_OP_TIMEOUT: Duration = Duration::from_secs(10);

/// Send a minimal HTTP/1.1 request (no body) and return the response status code with its body.
fn http_request_with_body(method: &str, path: &str) -> std::io::Result<(u16, String)> {
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
fn http_request_core(
    method: &str,
    path: &str,
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<(u16, String)> {
    let addr_str = bind_addr();
    let addr: SocketAddr = addr_str
        .parse()
        .expect("bind address is a valid socket address");
    let mut stream = std::net::TcpStream::connect_timeout(&addr, timeout)?;
    stream
        .set_read_timeout(Some(timeout))
        .expect("set read timeout on loopback TCP stream");
    stream
        .set_write_timeout(Some(timeout))
        .expect("set write timeout on loopback TCP stream");
    let payload = body.unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr_str}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(req.as_bytes())
        .expect("write HTTP request to local server");
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
    spawn_detached_with(|cmd| {
        cmd.arg("--interactive").env(DAEMONIZED_ENV, "1");
    })
}

/// Spawn a detached helper that stops the currently-running server and starts a fresh one,
/// returning the helper's PID. Used by the `/api/v1/restart` route and the `restart` MCP tool so the
/// daemon can be cycled from any surface, not just the CLI: the in-process server cannot rebind its
/// own port, so it delegates the stop-old-then-start-new dance to this separate process.
///
/// The helper is launched with the `--background` flag rather than the `restart` subcommand on
/// purpose: `moadim --background` ([`run_background`]) already stops a running instance before
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
fn spawn_detached_with(configure: impl FnOnce(&mut std::process::Command)) -> anyhow::Result<u32> {
    use std::process::{Command as Proc, Stdio};

    let exe = std::env::current_exe().expect("resolve current executable path");
    let log_path = crate::paths::daemon_log_file();
    std::fs::create_dir_all(log_path.parent().expect("daemon log path has a parent dir"))?;
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err = out
        .try_clone()
        .expect("clone log file handle for stderr redirect");

    let mut cmd = Proc::new(exe);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    configure(&mut cmd);
    detach(&mut cmd);

    #[allow(clippy::zombie_processes)]
    let child = cmd.spawn().expect("spawn detached moadim child process");
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
