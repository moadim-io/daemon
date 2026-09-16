//! Command-line interface: run-mode selection and background-process lifecycle.
//!
//! The `moadim` binary runs an HTTP/MCP/UI server. By default it starts that server **detached in
//! the background** and returns control to the shell — you then manage it from the client (the web
//! UI "STOP" button at the root URL) or with `moadim stop`. Pass `--interactive` to run it in the foreground
//! attached to the terminal instead (Ctrl-C to stop).

use std::time::Duration;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod wants_quiet;
pub(crate) use wants_quiet::*;
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod stop_json;
pub(crate) use stop_json::*;

/// Environment marker set on the backgrounded child so it knows it was spawned by the launcher.
pub(crate) const DAEMONIZED_ENV: &str = "MOADIM_DAEMONIZED";

/// Process exit code emitted by `status`/`cleanup` when no server is running, so callers can branch
/// on `$?` without parsing stdout. The success case (server reachable) exits `0`.
pub const EXIT_NOT_RUNNING: i32 = 3;

/// Process exit code for a usage error (an unknown/mistyped command or mode), following the common
/// CLI convention that a usage error exits `2` while an explicit `--help` exits `0`. Lets a wrapper
/// script, systemd unit, or CI step detect `moadim <typo>` instead of mistaking it for success.
pub const EXIT_USAGE: i32 = 2;

/// Map a server-liveness flag to the script-friendly process exit code: `0` when a server is
/// reachable, [`EXIT_NOT_RUNNING`] when it is not.
const fn liveness_exit_code(running: bool) -> i32 {
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
        /// detached in the background (mirrors `moadim -i`/`-f`).
        interactive: bool,
    },
    /// Ask a running background server to stop. `json` requests machine-readable output.
    ///
    /// Stops the daemon process only: any routine agent already running in a detached tmux
    /// session (issue #320) is left alive and keeps acting until it finishes on its own or the
    /// daemon is restarted and its watchdog/cleanup sweep reaps it.
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
    /// Print a routine's newest run log (`agent.log`) to stdout, by UUID. A top-level shorthand
    /// for `moadim routines logs <id>`, mirroring the `trigger`/`routines trigger` duality
    /// (issue #332).
    Logs {
        /// UUID of the routine whose log to print.
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
    /// Print a shell-completion script for `shell` (bash/zsh/fish/powershell/elvish) to stdout,
    /// or (when `shell` is missing or unrecognized) a usage error to stderr. See
    /// [`crate::cli::completions`].
    Completions(Option<String>),
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
pub(crate) const DATA_COMMANDS: &[&str] = &[
    "routines", "schedule", "agents", "enable", "disable", "export", "import",
];
include!("wants_json.rs");
