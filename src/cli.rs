//! Command-line interface: run-mode selection and background-process lifecycle.
//!
//! The `moadim` binary runs an HTTP/MCP/UI server. By default it starts that server **detached in
//! the background** and returns control to the shell — you then manage it from the client (the web
//! UI "STOP" button at the root URL) or with `moadim stop`. Pass `--interactive` to run it in the foreground
//! attached to the terminal instead (Ctrl-C to stop).

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

/// Environment marker set on the backgrounded child so it knows it was spawned by the launcher.
const DAEMONIZED_ENV: &str = "MOADIM_DAEMONIZED";

/// Process exit code emitted by `status`/`cleanup` when no server is running, so callers can branch
/// on `$?` without parsing stdout. The success case (server reachable) exits `0`.
pub const EXIT_NOT_RUNNING: i32 = 3;

/// Process exit code for a usage error (an unknown/mistyped command or mode), following the common
/// CLI convention that a usage error exits `2` while an explicit `--help` exits `0`. Lets a wrapper
/// script, systemd unit, or CI step detect `moadim <typo>` instead of mistaking it for success.
pub const EXIT_USAGE: i32 = 2;

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
    /// Stop a running background server (if any) and start a fresh instance. `json` requests
    /// machine-readable output; `quiet` suppresses the UI/stop/logs hint block (both ignored when
    /// `interactive` is set).
    Restart {
        /// Emit a machine-readable JSON object (`{"old":N|null,"new":N,"address":…}`) instead of the
        /// human-readable rotation line and hint block.
        json: bool,
        /// Print only the `restarted: pid <old> -> <new>` rotation line, suppressing the UI/stop/logs
        /// hint block. Ignored under `json`, which always prints its single object.
        quiet: bool,
        /// Start the fresh instance in the foreground, attached to the terminal, instead of
        /// detached in the background (mirrors `moadim -i`).
        interactive: bool,
    },
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
        /// When present, poll up to this many seconds for a server to become reachable instead of
        /// checking once, so scripts can block on startup rather than sleeping blindly.
        wait_secs: Option<u64>,
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
    /// Print usage help. Set by an explicit `help`/`-h`/`--help` request, which is a success:
    /// help goes to stdout and the process exits `0`.
    Help,
    /// An unrecognized first argument (a typo or unsupported command/mode). Carries the offending
    /// token so the dispatcher can print `unknown command: <arg>` to stderr and exit with
    /// [`EXIT_USAGE`], keeping a usage error distinct from an explicit, successful [`Command::Help`].
    Usage(String),
    /// Print the binary version.
    Version,
    /// A data-plane subcommand (`routines`, `agents`) handled by the clap-based
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
pub(crate) const DATA_COMMANDS: &[&str] = &["routines", "schedule", "agents", "enable", "disable"];

/// Parse CLI arguments (excluding the program name) into a [`Command`].
///
/// An unrecognized first argument maps to [`Command::Usage`] (a usage error written to stderr,
/// exiting [`EXIT_USAGE`]) rather than [`Command::Help`], so a typo like `moadim staus` is not
/// mistaken for a successful invocation. With no arguments the default is [`Command::Background`].
pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    let args: Vec<String> = args.into_iter().collect();
    match args.first().map(String::as_str) {
        Some(first) if DATA_COMMANDS.contains(&first) => Command::Data(args),
        Some("machine") => Command::Machine(args[1..].to_vec()),
        Some("restart") => Command::Restart {
            json: wants_json(&args[1..]),
            quiet: wants_quiet(&args[1..]),
            interactive: wants_interactive(&args[1..]),
        },
        Some("stop") => Command::Stop {
            json: wants_json(&args[1..]),
            quiet: wants_quiet(&args[1..]),
        },
        Some("status") => Command::Status {
            json: wants_json(&args[1..]),
            wait_secs: wants_wait(&args[1..]),
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
        None | Some("-b" | "--background" | "-d" | "--detach" | "--daemon") => Command::Background,
        Some(other) => Command::Usage(other.to_string()),
    }
}

/// Whether a `--json` flag appears among a command's trailing arguments, requesting
/// machine-readable output for `status`/`cleanup`/`stop`/`restart`.
fn wants_json(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--json")
}

/// Whether a `--quiet`/`-q` flag appears among a command's trailing arguments, requesting that
/// `stop`/`restart` suppress their human-readable output.
fn wants_quiet(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--quiet" || arg == "-q")
}

/// Whether a `--interactive`/`-i` flag appears among a command's trailing arguments, requesting
/// that `restart` bring the fresh instance up in the foreground instead of detached.
fn wants_interactive(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--interactive" || arg == "-i")
}

/// Default poll timeout for a bare `--wait` (no explicit seconds) on `status`.
const DEFAULT_WAIT_SECS: u64 = 30;

/// Whether `--wait` or `--wait=SECS` appears among `status`'s trailing arguments, requesting that
/// it poll for a server to come up instead of checking once. A bare `--wait` uses
/// [`DEFAULT_WAIT_SECS`]; `--wait=SECS` uses the given timeout. Returns `None` when neither form is
/// present, or `--wait=` is followed by something that does not parse as a `u64`.
fn wants_wait(rest: &[String]) -> Option<u64> {
    rest.iter().find_map(|arg| {
        if arg == "--wait" {
            Some(DEFAULT_WAIT_SECS)
        } else {
            arg.strip_prefix("--wait=")
                .and_then(|secs| secs.parse().ok())
        }
    })
}

/// Build the usage help text. Every flag listed here must stay in sync with the
/// aliases [`parse`] actually accepts; `cli_help_tests` asserts as much.
pub fn help_text() -> String {
    let bind_addr = bind_addr();
    format!(
        "moadim — routine scheduler with an MCP/REST API and a web control panel\n\
         \n\
         USAGE:\n\
         \x20   moadim [MODE]\n\
         \x20   moadim <COMMAND>\n\
         \n\
         MODES:\n\
         \x20   (default)              start the server in the background and exit\n\
         \x20   -i, --interactive      run in the foreground, attached to the terminal (Ctrl-C to stop); aliases: -f, --foreground\n\
         \x20   -b, --background       start the server detached in the background (explicit default); aliases: -d, --detach, --daemon\n\
         \n\
         COMMANDS:\n\
         \x20   restart [--json] [-q] [-i] stop a running server (if any) and start a fresh one\n\
         \x20                          (-q/--quiet: rotation line only; -i/--interactive: foreground)\n\
         \x20   stop [--json] [-q]     stop a running background server (-q/--quiet: no stdout)\n\
         \x20   status [--json] [--wait[=SECS]] show whether a server is running (--wait: poll until\n\
         \x20                          reachable or SECS elapse, default 30, instead of checking once)\n\
         \x20   cleanup [--json]       reap finished, expired routine workbenches now\n\
         \x20   trigger <id>           trigger a routine to run now, outside its schedule\n\
         \x20   install                register moadim as an OS service (launchd / systemd user)\n\
         \x20   uninstall              remove the OS service registration and the managed crontab block\n\
         \x20   machine <show|set|list> show/set this machine's identity, or list machines referenced\n\
         \x20   help, -h, --help       show this help\n\
         \x20   version, -V, --version show the version\n\
         \n\
         DATA COMMANDS (talk to the running server over HTTP; pass --help for flags):\n\
         \x20   routines  <create|list|get|update|replace|delete|trigger|logs|ical> ...\n\
         \x20   schedule  trigger <id> trigger a routine by ID (used by the routines crontab line)\n\
         \x20   enable <routine> [--json]   turn a routine on (set enabled=true) by id or slug\n\
         \x20   disable <routine> [--json]  turn a routine off (set enabled=false) by id or slug\n\
         \x20   agents                 list available agent keys\n\
         \n\
         Pass --json to `restart`/`stop`/`status`/`cleanup` for a single-line machine-readable object.\n\
         `status`/`cleanup`/`stop` exit 0 when a server is running and 3 when none is, so scripts\n\
         can branch on $? without parsing stdout.\n\
         \n\
         Once running, manage the server from the web client at http://{bind_addr}\n\
         (the STOP button) or with `moadim stop`."
    )
}

/// Report an unknown/mistyped command to **stderr** (not stdout) with a hint to run `moadim help`.
///
/// Kept off stdout so a script capturing a command's normal output never confuses this usage error
/// for real data; the caller pairs this with [`EXIT_USAGE`] so `$?` is non-zero.
pub fn print_usage_error(arg: &str) {
    eprintln!("moadim: unknown command: {arg}");
    eprintln!("Run `moadim help` for usage.");
}

/// Print usage help to stdout.
pub fn print_help() {
    println!("{}", help_text());
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

/// Stop a currently running background server, if any, printing the same status line used by
/// `restart` unless `quiet` suppresses it. Returns the PID of the server that was stopped, or
/// `None` if none was running.
///
/// Shared by [`restart`] (which spawns a fresh detached instance afterward) and the interactive
/// `restart -i` path in `main`, which brings the fresh instance up in the foreground instead.
pub(crate) fn stop_existing_for_restart(quiet: bool) -> anyhow::Result<Option<u32>> {
    if is_running() {
        let pid = read_pid_file();
        if !quiet {
            let suffix = pid
                .map(|process_id| format!(" (pid {process_id})"))
                .unwrap_or_default();
            println!("moadim is running{suffix}; stopping it");
        }
        crate::restart::stop_running_and_wait()?;
        Ok(pid)
    } else {
        if !quiet {
            println!("moadim is not running; starting a fresh instance");
        }
        Ok(None)
    }
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

/// Stop a running background server (if any) and start a fresh detached instance. With `json`,
/// emits a single machine-readable object (`{"old":N|null,"new":M}`) instead of the human-readable
/// lines.
///
/// Unlike [`run_background`], which restarts only as a side effect of being asked to start while
/// one is already up, this is the explicit "give me a clean process now" command: it stops the
/// running server when present, otherwise just starts one.
pub fn restart(json: bool, quiet: bool) -> anyhow::Result<()> {
    // Only the bare command narrates the stop/start step and prints the hint block; `--json` emits a
    // single object and `--quiet` prints just the rotation line.
    let old_pid = stop_existing_for_restart(json || quiet)?;
    let new_pid = spawn_detached()?;
    if json {
        println!("{}", restart_json(old_pid, new_pid));
    } else {
        // Headline the rotation so scripts/logs can see the process actually changed.
        println!("{}", restart_rotation_line(old_pid, new_pid));
        if !quiet {
            report_endpoints();
        }
    }
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

/// Render the `restart` result as a one-line JSON object:
/// `{"old":N|null,"new":N,"address":…}`. `old` is the PID of the server that was stopped (or `null`
/// when nothing was running, mirroring the `none` rendering in [`restart_rotation_line`]), `new` is
/// the freshly spawned PID, and `address` is the bound [`BIND_ADDR`] — matching the `address` field
/// every other `--json` lifecycle command surfaces.
fn restart_json(old: Option<u32>, new: u32) -> String {
    serde_json::json!({
        "old": old,
        "new": new,
        "address": bind_addr(),
    })
    .to_string()
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
/// `{"running":bool,"pid":N|null,"address":…}` — a subset of `status --json`'s shape (which
/// additionally folds in server-sourced `uptime_secs`/`version`; see
/// `status_and_stop_json_share_a_common_key_set`), so both can still be parsed uniformly on their
/// shared fields. `running` is `true` when a running server was asked to shut down, and `false`
/// when none was reachable. `pid` is the process that was stopped (read from the pid file before
/// the shutdown request), or `null` when no pid file was present. `address` is the bound address
/// the request was sent to ([`bind_addr`], honoring the `MOADIM_BIND_ADDR` override) so it stays
/// identical to `status --json` under a non-default bind.
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
            let freed_bytes = parse_freed_bytes(&body).unwrap_or(0);
            if json {
                println!("{}", cleanup_json(removed, freed_bytes, true));
            } else {
                let plural = if removed == 1 { "" } else { "es" };
                println!(
                    "cleanup removed {removed} workbench{plural} (freed {})",
                    humanize_bytes(freed_bytes)
                );
            }
            Ok(liveness_exit_code(true))
        }
        Ok((status, _)) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            if json {
                println!("{}", cleanup_json(0, 0, false));
            } else {
                println!("moadim is not running");
            }
            Ok(liveness_exit_code(false))
        }
    }
}

/// Render a byte count as a short human-readable size using 1024-based units. Values under 1 KiB
/// are shown as a bare integer (`512 B`); larger values use one decimal place (`12.4 MB`). Caps at
/// TB so the unit table can't be indexed out of range.
fn humanize_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
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
/// When `wait_secs` is `Some`, and no server answers on the first check, polls `GET /health`
/// every `WAIT_POLL_INTERVAL` until one does or the timeout elapses, so a caller can block on
/// startup (`moadim & moadim status --wait`) instead of sleeping blindly before probing.
///
/// Returns the process exit code to surface: `0` when a server is reachable, and
/// [`EXIT_NOT_RUNNING`] when not (including after a `--wait` timeout), so scripts can branch on
/// `$?` without parsing stdout.
pub fn status(json: bool, wait_secs: Option<u64>) -> anyhow::Result<i32> {
    let mut running = is_running();
    if !running {
        if let Some(secs) = wait_secs {
            running = wait_until(is_running, Duration::from_secs(secs));
        }
    }
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
/// `{"running":bool,"removed":N,"freed_bytes":N,"address":…}`. `removed`/`freed_bytes` are `0` when
/// the server is not running (`running:false`). `address` is the effective bound [`bind_addr`] the
/// request was sent to, matching `status --json`/`stop --json`'s object shape so every `--json`
/// command surfaces the endpoint it talked to. The pre-existing `running`/`removed` keys are
/// preserved; `freed_bytes` is additive.
fn cleanup_json(removed: usize, freed_bytes: u64, running: bool) -> String {
    serde_json::json!({
        "running": running,
        "removed": removed,
        "freed_bytes": freed_bytes,
        "address": bind_addr(),
    })
    .to_string()
}

#[path = "cli_system.rs"]
mod cli_system;
pub use cli_system::{clear_pid_file, spawn_restart, write_pid_file};
pub(crate) use cli_system::{http_request, http_request_json, is_running, read_pid_file};
use cli_system::{
    http_request_with_body, parse_freed_bytes, parse_removed_count, paths_daemon_log,
    spawn_detached, wait_until,
};
#[cfg(test)]
pub(crate) use cli_system::{parse_body, parse_status_code};

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;

#[cfg(test)]
#[path = "cli_cleanup_bytes_tests.rs"]
mod cli_cleanup_bytes_tests;

#[cfg(test)]
#[path = "cli_help_tests.rs"]
mod cli_help_tests;

#[cfg(test)]
#[path = "cli_json_tests.rs"]
mod cli_json_tests;

#[cfg(test)]
#[path = "cli_spawn_tests.rs"]
mod cli_spawn_tests;
