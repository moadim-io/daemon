#![deny(warnings)]
//! Moadim server binary. Runs the Axum HTTP server with REST and MCP transports.

/// Command-line interface and background-process lifecycle.
mod cli;
mod cron_jobs;
mod error;
/// Server filesystem location helpers.
mod filesystem;
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
/// TOML-backed job persistence.
mod storage;
/// Forward sync of managed jobs into the OS crontab (reverse sync is implemented
/// but not wired up — see the `sync` module docs and issue #218).
mod sync;
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
        cli::Command::Status { json } => std::process::exit(cli::status(json)?),
        cli::Command::Cleanup { json } => std::process::exit(cli::cleanup(json)?),
        cli::Command::Stop { json } => std::process::exit(cli::stop(json)?),
        cli::Command::Background => cli::run_background(),
        cli::Command::Restart => cli::restart(),
        cli::Command::Install => service::install(),
        cli::Command::Uninstall => service::uninstall(),
        cli::Command::Foreground => run_server().await,
    }
}

/// Run the HTTP/MCP/UI server in the foreground until a termination signal or the `/shutdown` route
/// stops it. Records this process's PID so `moadim stop`/`status` can find it, and clears it on exit.
async fn run_server() -> anyhow::Result<()> {
    routines::ensure_default_agents();
    let store = storage::load_store();
    // Rename any prompt.txt sidecars to prompt.md before rewriting run.sh scripts; otherwise the
    // first cron trigger after upgrade would fail on the cp step.
    routine_storage::migrate_prompt_files();
    // Move legacy UUID-named routine dirs to the current slug-based layout before loading, so the
    // store reflects the canonical dirs the crontab sync and run.sh `cp prompt.md` both target.
    routine_storage::migrate_routine_dirs();
    let routines = routine_storage::load_store();
    // Seed any missing built-in default routines (e.g. the daily moadim cargo update check) so a
    // fresh install ships with them, and a default deleted while stopped is restored. Existing
    // routines are never overwritten. Must run before the crontab sync so the defaults schedule.
    routines::ensure_default_routines(&routines);
    // The crontab sync writes only run.sh; re-persist so every routine also has its routine.toml +
    // prompt.md sidecar in the slug dir, healing dirs left with run.sh but no prompt (otherwise the
    // cron `cp prompt.md` fails and the agent launches with an empty prompt).
    routine_storage::repersist_routines(&routines);
    // Re-sync routines to the crontab on startup; otherwise a block that went stale (e.g. emptied
    // by an earlier run before agent configs existed) would never be regenerated until the next
    // create/update/delete, leaving scheduled routines silently un-fired.
    if let Err(err) = sync::routines::sync_routines_to_crontab(&routines) {
        log::warn!("startup crontab sync failed: {err}");
    }
    let listener = tokio::net::TcpListener::bind(cli::bind_addr()).await?;
    cli::write_pid_file()?;
    let result =
        routes::http::run_with_listener_until(store, routines, listener, termination_signal())
            .await;
    cli::clear_pid_file();
    result
}

/// Resolves when the process receives a termination signal (SIGINT/Ctrl-C, or SIGTERM on Unix),
/// driving a graceful shutdown so the pid file is cleared even when stopped from the terminal.
async fn termination_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
