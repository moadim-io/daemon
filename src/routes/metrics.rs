//! `GET /api/v1/metrics` — Prometheus text-exposition metrics: run counts by outcome, run
//! duration distribution, live agent sessions, workbench disk usage, and cleanup-sweep totals
//! (issue #414).
//!
//! No MCP tool mirrors this route — a raw Prometheus text dump isn't a meaningful
//! agent-callable action the way `health`/`restart`/`list_agents` are — so per
//! `src/routes/CONTRIBUTING.md`'s "small routes ... can go straight into `http.rs`" guidance it
//! lives in its own flat file rather than the `logic.rs`/`http.rs`/`mcp.rs` folder split those
//! routes use.
//!
//! Every series here is derived at scrape time from state the daemon already tracks durably:
//! run counts and durations come from [`crate::routines::svc_list_all_runs`] (the same merged
//! live-workbench + `runs.log` view the "recent runs" REST/UI surface already reads), active
//! sessions from the same live tmux count the concurrency cap uses, and workbench disk usage
//! from a walk of `~/.moadim/workbenches/`. That keeps this endpoint a read model over existing
//! ground truth rather than a second, parallel set of counters that could drift from it. The one
//! exception is `moadim_cleanup_removed_total`/`moadim_cleanup_freed_bytes_total`: a cleanup
//! sweep leaves no durable per-sweep log to replay, so those two are process-lifetime atomics
//! (see `crate::routines::cleanup::counters`) incremented at the one function
//! (`cleanup_expired_workbenches`) both the periodic sweep and the on-demand
//! `POST /routines/cleanup` route already funnel through.

use std::fmt::Write as _;

use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;

use crate::routes::http::AppState;
use crate::routines::{svc_list_all_runs, FleetRunSummary, RunStatus};
use crate::utils::time::now_secs;

/// `Content-Type` for the Prometheus text exposition format (version `0.0.4`).
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// Upper bounds (seconds) of the `moadim_run_duration_seconds` histogram's finite buckets,
/// narrowest first — chosen to span a quick agent turn (a few seconds) through an hour-long one.
/// Prometheus histograms always carry an implicit final `+Inf` bucket beyond these.
const DURATION_BUCKETS_SECS: [u64; 9] = [5, 15, 30, 60, 120, 300, 600, 1800, 3600];

/// Everything [`render`] needs to produce the exposition text, gathered once by [`metrics`] so
/// the formatting logic itself stays a pure function of its inputs (and independently testable).
struct MetricsSnapshot<'input> {
    /// Seconds since the daemon started.
    uptime_secs: u64,
    /// Daemon version (`CARGO_PKG_VERSION`).
    version: &'input str,
    /// Short git commit SHA the daemon was built from.
    git_sha: &'input str,
    /// Resolved name of this machine.
    machine: &'input str,
    /// Number of live tmux agent sessions right now.
    active_sessions: usize,
    /// Total size in bytes of the workbench tree on disk.
    workbench_bytes: u64,
    /// Total size in bytes of the repository mirror cache tree (`{config_dir}/cache/`) on disk —
    /// never pruned before issue #1425, so previously invisible without walking the filesystem
    /// by hand.
    repo_cache_bytes: u64,
    /// Every run across every routine, live and historical (see
    /// [`crate::routines::svc_list_all_runs`]).
    runs: &'input [FleetRunSummary],
    /// Workbenches removed by cleanup sweeps since this process started.
    cleanup_removed_total: u64,
    /// Bytes freed by cleanup sweeps since this process started.
    cleanup_freed_bytes_total: u64,
}

/// `GET /api/v1/metrics` — Prometheus text-exposition metrics. See the module doc for how each
/// series is derived; `/health` remains the cheap liveness probe, this is the richer surface.
#[utoipa::path(get, path = "/metrics",
    responses((status = 200, description = "Prometheus text exposition format (version 0.0.4)", body = str)))]
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let runs = svc_list_all_runs(&state.routines, Some(usize::MAX));
    let machine = crate::machine::current_machine();
    let (cleanup_removed_total, cleanup_freed_bytes_total) =
        crate::routines::cleanup_sweep_totals();
    let snapshot = MetricsSnapshot {
        uptime_secs: now_secs().saturating_sub(state.uptime_start),
        version: crate::build_info::VERSION,
        git_sha: crate::build_info::GIT_SHA,
        machine: machine.as_str(),
        active_sessions: crate::routines::tmux_session_count(crate::routines::TMUX_SESSION_PREFIX),
        workbench_bytes: crate::routines::workbenches_total_bytes(),
        repo_cache_bytes: crate::routines::repo_cache_total_bytes(),
        runs: &runs,
        cleanup_removed_total,
        cleanup_freed_bytes_total,
    };
    ([(CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)], render(&snapshot))
}

/// Count of runs at each [`RunStatus`], tallied by [`render`] from a [`MetricsSnapshot`]'s runs.
#[derive(Default)]
struct RunStatusCounts {
    /// Runs whose agent process exited `0`.
    success: u64,
    /// Runs whose agent process exited non-zero.
    failed: u64,
    /// Runs whose tmux session is still alive.
    running: u64,
    /// Runs whose session ended with no exit code recorded (killed, crashed, or from a build
    /// predating exit-code capture).
    unknown: u64,
}

/// Escape a value for use inside a Prometheus text-exposition label (`name="value"`): backslash
/// and double-quote must themselves be backslash-escaped, and a literal newline is written as the
/// two-character `\n` sequence, per the exposition format's label-value grammar.
///
/// `machine` (set via `moadim machine set <name>` or `MOADIM_MACHINE`, see `crate::machine`) is
/// the only label value in this file sourced from free-form user input — trimmed but otherwise
/// unrestricted — so it's the only one that needs this: an operator-chosen name containing `"` or
/// `\` would otherwise emit unparseable exposition text and break the whole scrape, not just this
/// one line. `version`/`git_sha` are compile-time constants and never need it.
fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
include!("metrics_part2.rs");
