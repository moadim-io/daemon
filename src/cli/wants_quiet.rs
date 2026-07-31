#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Whether a `--quiet`/`-q` flag appears among a command's trailing arguments, requesting that
/// `stop`/`restart` suppress their human-readable output.
pub(crate) fn wants_quiet(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--quiet" || arg == "-q")
}

/// Whether a `--interactive`/`-i` flag (or its `--foreground`/`-f` alias, matching the top-level
/// start flags) appears among a command's trailing arguments, requesting that `restart` bring the
/// fresh instance up in the foreground instead of detached.
pub(crate) fn wants_interactive(rest: &[String]) -> bool {
    rest.iter()
        .any(|arg| arg == "--interactive" || arg == "-i" || arg == "--foreground" || arg == "-f")
}

/// Default poll timeout for a bare `--wait` (no explicit seconds) on `status`.
pub(crate) const DEFAULT_WAIT_SECS: u64 = 30;

/// Whether `--wait` or `--wait=SECS` appears among `status`'s trailing arguments, requesting that
/// it poll for a server to come up instead of checking once. A bare `--wait` uses
/// [`DEFAULT_WAIT_SECS`]; `--wait=SECS` uses the given timeout. Returns `None` when neither form is
/// present, or `--wait=` is followed by something that does not parse as a `u64`.
pub(crate) fn wants_wait(rest: &[String]) -> Option<u64> {
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
         \x20                          (-q/--quiet: rotation line only; -i/--interactive: run the\n\
         \x20                          fresh instance in the foreground, attached to the terminal\n\
         \x20                          (Ctrl-C to stop); aliases: -f, --foreground)\n\
         \x20   stop [--json] [-q]     stop a running background server, also killing any\n\
         \x20                          in-flight routine tmux sessions (-q/--quiet: no stdout)\n\
         \x20   status [--json] [--wait[=SECS]] show whether a server is running (--wait: poll until\n\
         \x20                          reachable or SECS elapse, default 30, instead of checking once)\n\
         \x20   cleanup [--json]       reap finished, expired routine workbenches now\n\
         \x20   trigger <id>           trigger a routine to run now, outside its schedule\n\
         \x20   logs <id>              print a routine's newest run log (agent.log) to stdout\n\
         \x20   install                register moadim as an OS service (launchd / systemd user)\n\
         \x20   uninstall              remove the OS service registration and the managed crontab block\n\
         \x20   machine <show|set|list> show/set this machine's identity, or list machines referenced\n\
         \x20   completions <shell>    print a completion script for bash/zsh/fish/powershell/elvish\n\
         \x20                          (e.g. `moadim completions zsh > _moadim`)\n\
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
         `stop` only stops the daemon process; a routine agent already running in its own detached\n\
         tmux session keeps running until it finishes or a later daemon start reaps it.\n\
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
include!("ensure_not_running_for_foreground.rs");
