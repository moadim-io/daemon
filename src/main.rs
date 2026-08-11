#![deny(warnings)]
// Forbid `.unwrap()` in production code so a poisoned lock or other panic
// cannot take the daemon down. Tests use `.unwrap()` freely (panicking is the
// desired failure mode there), so the lint is scoped to non-test builds.
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
//! Moadim server binary. Runs the Axum HTTP server with REST and MCP transports.

/// Compile-time build provenance (crate version + git commit/date).
mod build_info;
/// Command-line interface and background-process lifecycle.
mod cli;
/// Data-plane CLI subcommands (clap) that drive the running server over HTTP.
mod commands;
mod error;
/// Offline `moadim export`/`import`: back up and restore the tracked config as a JSON bundle.
mod export_import;
/// Server filesystem location helpers.
mod filesystem;
/// Global lock sentinel that halts all routine scheduling and triggers without modifying routine
/// enabled states.
mod global_lock;
/// `log` backend initialization: human-readable by default, opt-in JSON via `MOADIM_LOG_FORMAT`.
mod logging;
/// Machine identity for multi-machine deployments (per-machine routine/job targeting).
mod machine;
/// Axum middleware stack.
mod middlewares;
mod openapi;
/// Filesystem path builders for the jobs directory.
mod paths;
/// Replace an already-running daemon with a fresh process on launch.
mod restart;
/// HTTP and MCP route definitions.
mod routes;
/// TOML-backed routine persistence.
mod routine_storage;
/// Routine (agent-driven job) data model, service layer, and handlers.
mod routines;
/// `moadim install` / `uninstall`: register the daemon as an OS service.
mod service;
/// Forward sync of managed routines into the OS crontab.
mod sync;
/// Host power-state detector for conservative routine launch throttling.
mod system_power;
#[cfg(test)]
mod test_fixtures;
/// Shared utility functions.
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match cli::parse(std::env::args().skip(1)) {
        cli::Command::Help => {
            cli::print_help();
            Ok(())
        }
        cli::Command::Version => {
            cli::print_version();
            Ok(())
        }
        cli::Command::Usage(arg) => {
            cli::print_usage_error(&arg);
            std::process::exit(cli::EXIT_USAGE);
        }
        cli::Command::Status { json, wait_secs } => {
            std::process::exit(cli::status(json, wait_secs)?)
        }
        cli::Command::Cleanup { json } => std::process::exit(cli::cleanup(json)?),
        cli::Command::Stop { json, quiet } => std::process::exit(cli::stop(json, quiet)?),
        cli::Command::Trigger { id } => std::process::exit(cli::trigger(&id)?),
        cli::Command::Logs { id } => std::process::exit(cli::logs(&id)?),
        cli::Command::Background => cli::run_background(),
        cli::Command::Restart {
            json,
            quiet,
            interactive: false,
        } => cli::restart(json, quiet),
        cli::Command::Restart {
            interactive: true, ..
        } => {
            cli::stop_existing_for_restart(false)?;
            run_server().await
        }
        cli::Command::Install => service::install(),
        cli::Command::Uninstall => uninstall(),
        cli::Command::Completions(shell) => std::process::exit(cli::completions(shell.as_deref())),
        cli::Command::Data(args) => std::process::exit(commands::run(args)),
        cli::Command::Machine(args) => std::process::exit(machine::run(&args)),
        cli::Command::Foreground => {
            cli::ensure_not_running_for_foreground()?;
            run_server().await
        }
    }
}

/// `moadim uninstall`: tear down everything install/usage added — the OS service
/// registration AND the managed crontab block the daemon wrote. Without the
/// crontab step, `cron` keeps firing routines against a removed daemon (#380).
///
/// Both steps are best-effort and independent: a failure (or unsupported-platform
/// error) in the service step is reported but does not skip the crontab cleanup,
/// and the command still succeeds so a partial install can always be torn down.
fn uninstall() -> anyhow::Result<()> {
    if let Err(err) = service::uninstall() {
        eprintln!("moadim: service uninstall step failed: {err}");
    }
    match sync::clear_managed_crontab_blocks() {
        Ok(0) => println!("moadim: no managed crontab entries to remove"),
        Ok(1) => println!("moadim: removed 1 managed crontab entry"),
        Ok(n) => println!("moadim: removed {n} managed crontab entries"),
        Err(err) => eprintln!("moadim: crontab cleanup failed: {err}"),
    }
    Ok(())
}
include!("run_server.rs");
