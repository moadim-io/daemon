
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
        // `logs <id>` mirrors `trigger <id>`: without an id there is nothing to print, so fall
        // back to help rather than silently no-op.
        Some("logs") => match args.get(1) {
            Some(id) => Command::Logs { id: id.clone() },
            None => Command::Help,
        },
        Some("install") => Command::Install,
        Some("uninstall") => Command::Uninstall,
        Some("completions") => Command::Completions(args.get(1).cloned()),
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

#[path = "bind.rs"]
mod cli_bind;
pub use cli_bind::{bind_addr, validated_bind_addr, BIND_ADDR};
pub(crate) use cli_bind::api_token;
#[cfg(test)]
pub(crate) use cli_bind::{
    bind_addr_is_loopback, classify_bind, remote_bind_allowed, BindDecision, API_TOKEN_ENV,
    BIND_ADDR_ENV,
};

#[path = "query.rs"]
mod cli_query;
pub use cli_query::{cleanup, logs, status, trigger};
#[cfg(test)]
use cli_query::{
    cleanup_json, fetch_health, humanize_bytes, parse_health, status_json, CrontabSyncInfo, HealthInfo,
    CRONTAB_SYNC_RECOVERY_HINT,
};

#[path = "system.rs"]
mod cli_system;
pub(crate) use cli_system::{clear_pid_file, spawn_restart, write_pid_file};
pub(crate) use cli_system::{http_request, http_request_json, is_running, read_pid_file};
use cli_system::{
    http_request_with_body, parse_freed_bytes, parse_removed_count, paths_daemon_log,
    spawn_detached, wait_until,
};
#[cfg(test)]
pub(crate) use cli_system::{parse_body, parse_status_code, DAEMON_LOG_MAX_BYTES};
pub(crate) use cli_system::{rotate_daemon_log_if_due, LOG_ROTATION_CHECK_INTERVAL};

#[path = "restart.rs"]
mod cli_restart;
pub use cli_restart::restart;
use cli_restart::start_detached_and_report;
#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
use cli_restart::{maybe_hint_install, should_hint_install};
#[cfg(test)]
use cli_restart::{restart_json, restart_rotation_line};

#[path = "completions.rs"]
mod cli_completions;
pub use cli_completions::completions;
#[cfg(test)]
use cli_completions::{build_cli, write_completions};

#[cfg(test)]
#[path = "tests.rs"]
mod cli_tests;

#[cfg(test)]
#[path = "completions_tests.rs"]
mod cli_completions_tests;

#[cfg(test)]
#[path = "cleanup_bytes_tests.rs"]
mod cli_cleanup_bytes_tests;

#[cfg(test)]
#[path = "help_tests.rs"]
mod cli_help_tests;

#[cfg(test)]
#[path = "json_tests.rs"]
mod cli_json_tests;

#[cfg(test)]
#[path = "status_crontab_sync_tests.rs"]
mod cli_status_crontab_sync_tests;

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod cli_spawn_tests;

#[cfg(test)]
#[path = "spawn_error_tests.rs"]
mod cli_spawn_error_tests;
