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
mod cron_jobs;
mod error;
/// Server filesystem location helpers.
mod filesystem;
/// Global lock sentinel that halts all routine scheduling and triggers without modifying routine
/// enabled states.
mod global_lock;
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
        cli::Command::Stop { json, quiet } => std::process::exit(cli::stop(json, quiet)?),
        cli::Command::Trigger { id } => std::process::exit(cli::trigger(id)?),
        cli::Command::Enable { id, json } => std::process::exit(cli::enable(id, json)?),
        cli::Command::Disable { id, json } => std::process::exit(cli::disable(id, json)?),
        cli::Command::Background => cli::run_background(),
        cli::Command::Restart => cli::restart(),
        cli::Command::Install => service::install(),
        cli::Command::Uninstall => uninstall(),
        cli::Command::Data(args) => std::process::exit(commands::run(args)),
        cli::Command::Machine(args) => std::process::exit(machine::run(&args)),
        cli::Command::Foreground => run_server().await,
    }
}

/// `moadim uninstall`: tear down everything install/usage added — the OS service
/// registration AND the managed crontab blocks the daemon wrote. Without the
/// crontab step, `cron` keeps firing routines/jobs against a removed daemon (#380).
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

/// Run the HTTP/MCP/UI server in the foreground until a termination signal or the `/shutdown` route
/// stops it. Records this process's PID so `moadim stop`/`status` can find it, and clears it on exit.
async fn run_server() -> anyhow::Result<()> {
    // Initialize the logging backend so the `log::*` call sites across the daemon actually emit;
    // without an installed backend the `log` facade is a silent no-op and startup, crontab-sync,
    // and HTTP-request diagnostics are dropped. Defaults to the `info` level and is overridable via
    // `RUST_LOG`. A detached daemon redirects stderr to its log file, so these lines land there
    // with timestamps and levels. Use `try_init` to avoid panicking if a backend is already set.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
    // tmux is a hard runtime dependency: every routine agent launches via `tmux new-session`. When
    // it is missing the launch command silently no-ops (the statements are `;`-joined), so warn
    // loudly at startup rather than letting scheduled runs vanish. Also surfaced in `GET /health`.
    if !routines::tmux_available() {
        log::warn!(
            "tmux not found on PATH; scheduled routine runs will silently fail to launch their \
             agent. Install tmux (e.g. `brew install tmux` or `apt install tmux`)."
        );
    }
    routines::ensure_default_agents();
    let store = storage::load_store();
    // Rename any prompt.txt sidecars to prompt.md before the crontab resync; otherwise the first
    // cron trigger after upgrade would fail on the launch command's `cp prompt.md` step.
    routine_storage::migrate_prompt_files();
    // Move legacy UUID-named routine dirs to the current slug-based layout before loading, so the
    // store reflects the canonical dirs the crontab sync and the launch command's `cp prompt.md`
    // both target.
    routine_storage::migrate_routine_dirs();
    let routines = routine_storage::load_store();
    // Seed any missing built-in default routines (e.g. the daily moadim cargo update check) so a
    // fresh install ships with them, and a default deleted while stopped is restored. Existing
    // routines are never overwritten. Must run before the crontab sync so the defaults schedule.
    routines::ensure_default_routines(&routines);
    // Re-persist so every routine has its routine.toml + prompt.md sidecar in the slug dir (and any
    // stale legacy run.sh is removed), healing dirs left without a prompt (otherwise the launch
    // command's `cp prompt.md` fails and the agent launches with an empty prompt).
    routine_storage::repersist_routines(&routines);
    // Re-sync routines to the crontab on startup; otherwise a block that went stale (e.g. emptied
    // by an earlier run before agent configs existed) would never be regenerated until the next
    // create/update/delete, leaving scheduled routines silently un-fired.
    if let Err(err) = sync::routines::sync_routines_to_crontab(&routines) {
        log::warn!("startup crontab sync failed: {err}");
    }
    // Likewise re-sync managed cron-jobs to the crontab on startup, mirroring the routines sync
    // above; otherwise a lost or emptied block (manual `crontab -e`/`crontab -r`, an OS migration,
    // or a marker collision) leaves every managed job silently un-fired until the next job
    // create/update/delete. `sync_to_crontab` is idempotent, so this is a no-op read on a healthy
    // crontab.
    if let Err(err) = sync::sync_to_crontab(&store) {
        log::warn!("startup crontab sync (cron-jobs) failed: {err}");
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
