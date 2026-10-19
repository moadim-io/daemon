
/// Run the HTTP/MCP/UI server in the foreground until a termination signal or the `/shutdown` route
/// stops it. Records this process's PID so `moadim stop`/`status` can find it, and clears it on exit.
async fn run_server() -> anyhow::Result<()> {
    // Initialize the logging backend so the `log::*` call sites across the daemon actually emit;
    // without an installed backend the `log` facade is a silent no-op and startup, crontab-sync,
    // and HTTP-request diagnostics are dropped. A detached daemon redirects stderr to its log
    // file, so these lines land there with timestamps and levels. See `logging` for format
    // selection (`MOADIM_LOG_FORMAT`) and level filtering (`RUST_LOG`).
    logging::init();
    // tmux is a hard runtime dependency: every routine agent launches via `tmux new-session`. When
    // it is missing the launch command silently no-ops (the statements are `;`-joined), so warn
    // loudly at startup rather than letting scheduled runs vanish. Also surfaced in `GET /health`.
    if !routines::tmux_available() {
        log::warn!(
            "tmux not found on PATH; scheduled routine runs will silently fail to launch their \
             agent. Install tmux (e.g. `brew install tmux` or `apt install tmux`)."
        );
    }
    // python3 is a hard dependency of the built-in `claude` agent's `setup` step (workspace-trust
    // seeding). When it is missing, that step fails and the routine's agent never actually
    // launches, yet nothing else surfaces the failure — the routine still shows a healthy status
    // (issue #404). Warn at startup, same as the tmux check above; also surfaced in `GET /health`.
    if !routines::agent_command_available("python3") {
        log::warn!(
            "python3 not found on PATH; the built-in `claude` agent's setup step requires it to \
             pre-seed workspace-trust state, so routines using that agent will silently fail to \
             launch. Install python3, or use a different agent."
        );
    }
    routines::ensure_default_agents();
    // Rename any prompt.txt sidecars to prompt.md before the crontab resync; otherwise the first
    // cron trigger after upgrade would fail on the launch command's `cp prompt.compiled.local.md`
    // step.
    routine_storage::migrate_prompt_files();
    // Move each routine's prompt file(s) into its prompts/ subfolder, and extract the raw prompt
    // out of routine.toml into prompts/prompt.pure.md. Must run before migrate_routine_dirs and
    // load_store, which both read the prompt from the new sidecar location.
    routine_storage::migrate_prompts_to_subfolder();
    // Rename the compiled-prompt sidecar from the legacy prompt.compiled.md to
    // prompt.compiled.local.md so it matches the *.local.* gitignore pattern instead of relying on
    // an explicit entry (issue #1046). Must run after migrate_prompts_to_subfolder (which is what
    // lands the legacy filename in prompts/) and before load_store.
    routine_storage::migrate_compiled_prompt_filename();
    // Move legacy UUID-named routine dirs to the current slug-based layout before loading, so the
    // store reflects the canonical dirs the crontab sync and the launch command's
    // `cp prompt.compiled.local.md` both target.
    routine_storage::migrate_routine_dirs();
    // Migrate per-routine trigger timestamps from legacy TOML sidecars (scheduled.local.toml,
    // last_manual_trigger_at in state.local.toml) into the new append-only log files
    // (scheduled.log, manual.log). Must run before load_store so the first load already reads from
    // the log files.
    routine_storage::migrate_trigger_logs();
    let routines = routine_storage::load_store();
    // Seed any missing built-in default routines (e.g. the daily moadim cargo update check) so a
    // fresh install ships with them, and a default deleted while stopped is restored. Existing
    // routines are never overwritten. Must run before the crontab sync so the defaults schedule.
    routines::ensure_default_routines(&routines);
    // Re-persist so every routine has its routine.toml + schedule.cron + prompts/ sidecars in the
    // slug dir (and any stale legacy run.sh is removed), healing dirs left without a prompt or
    // cron file (otherwise the launch command's `cp prompt.compiled.local.md` fails and the agent
    // launches with an empty prompt).
    routine_storage::repersist_routines(&routines);
    // Register the current enabled routines with the daemon-owned scheduler. Routine mutations
    // request the same library scheduler to rebuild its jobs, so no platform relies on OS crontab.
    if let Err(err) = sync::routines::sync_routines_to_crontab(&routines) {
        log::warn!("startup routine scheduler sync failed: {err}");
    }
    #[cfg(not(test))]
    let _routine_scheduler = routine_scheduler::spawn(routines.clone());
    let bind_addr = cli::validated_bind_addr().map_err(anyhow::Error::msg)?;
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    cli::write_pid_file()?;
    let result =
        routes::http::run_with_listener_until(routines, listener, termination_signal()).await;
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
