//! Global cap on concurrently-running routine agent sessions (#335).
//!
//! Routines are driven by the OS crontab, so cron fires for many *different* routines naturally
//! align on the same minute boundary (e.g. `*/5 * * * *`, `0 * * * *`, …) — nothing bounds how many
//! agent sessions launch on the same tick, a thundering herd that can exhaust host CPU/RAM or burst
//! past a provider's API rate limit. This is distinct from the per-routine overlap guard
//! (`cleanup::tmux_session_prefix_alive`, #514): that only stops one routine from stacking on top of
//! its own still-running fire, and does nothing to bound the total number of *different* routines
//! running at once.
//!
//! `service_trigger::spawn_routine_command` checks [`max_concurrent_runs`] against the live session
//! count (`cleanup::tmux_session_count`, keyed on the shared `moadim-` prefix every routine's tmux
//! session name begins with) before launching, and skips the fire — logging a warning — rather than
//! queueing it, the same non-fatal skip shape the overlap guard above already uses.

/// Env var naming the global concurrency cap. Unset or unparsable falls back to
/// [`DEFAULT_MAX_CONCURRENT_RUNS`]. `0` (unset or explicit) means unbounded — the same convention
/// `MOADIM_MAX_WORKBENCH_DISK_BYTES` uses.
pub(crate) const MAX_CONCURRENT_RUNS_ENV: &str = "MOADIM_MAX_CONCURRENT_RUNS";

/// Default cap applied when [`MAX_CONCURRENT_RUNS_ENV`] is unset/unparsable: `0`, i.e. no limit.
const DEFAULT_MAX_CONCURRENT_RUNS: usize = 0;

/// The configured global concurrency cap: how many routine agent sessions may be alive at once
/// before a new fire is skipped instead of launched. `0` means unbounded.
///
/// Precedence: [`MAX_CONCURRENT_RUNS_ENV`] (ops/CI) > the UI/REST-configured override persisted in
/// `machine.local.toml` (`crate::machine::max_concurrent_runs_override`, issue #1155) >
/// [`DEFAULT_MAX_CONCURRENT_RUNS`].
pub(crate) fn max_concurrent_runs() -> usize {
    std::env::var(MAX_CONCURRENT_RUNS_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .or_else(crate::machine::max_concurrent_runs_override)
        .unwrap_or(DEFAULT_MAX_CONCURRENT_RUNS)
}

#[cfg(test)]
#[path = "concurrency_cap_tests.rs"]
mod concurrency_cap_tests;
