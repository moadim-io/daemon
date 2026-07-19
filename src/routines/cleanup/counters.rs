//! Process-lifetime counters backing the cleanup-sweep metrics (`moadim_cleanup_removed_total`,
//! `moadim_cleanup_freed_bytes_total`) exposed by `GET /api/v1/metrics`
//! (`crate::routes::metrics`). Both the periodic background sweep and the on-demand
//! `POST /routines/cleanup` route funnel through `cleanup_expired_workbenches` (see
//! `super::cleanup_expired_workbenches`), so recording a sweep once there covers both triggers.
//!
//! Deliberately in-memory rather than persisted: like the run-duration histogram in
//! `crate::routes::metrics`, these are process-lifetime counters that reset to `0` on a daemon
//! restart, which matches how an operator reads a Prometheus counter after a scrape target
//! restarts (a reset, not a decrease).

use std::sync::atomic::{AtomicU64, Ordering};

/// Total workbenches removed by every cleanup sweep since this process started.
static REMOVED_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Total bytes freed by every cleanup sweep since this process started.
static FREED_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Add one sweep's `removed`/`freed_bytes` ([`super::ReapStats`]) to the running totals.
pub(crate) fn record_sweep(removed: u64, freed_bytes: u64) {
    REMOVED_TOTAL.fetch_add(removed, Ordering::Relaxed);
    FREED_BYTES_TOTAL.fetch_add(freed_bytes, Ordering::Relaxed);
}

/// Current `(removed_total, freed_bytes_total)` snapshot, read by `GET /api/v1/metrics`.
pub(crate) fn totals() -> (u64, u64) {
    (
        REMOVED_TOTAL.load(Ordering::Relaxed),
        FREED_BYTES_TOTAL.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
#[path = "counters_tests.rs"]
mod counters_tests;
