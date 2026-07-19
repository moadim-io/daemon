#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::{record_sweep, totals};

// These counters are process-global (`static AtomicU64`), and `cargo test` runs the whole
// binary's tests concurrently on multiple threads within one process — including other cleanup
// tests that call `cleanup_expired_workbenches` (and so `record_sweep`) directly. Asserting an
// exact before/after delta would be flaky under that concurrency, so this only checks the
// monotonic "increased by at least what we just recorded" property, which holds regardless of
// what any other test thread adds concurrently.
#[test]
fn record_sweep_increments_totals_by_at_least_the_recorded_amount() {
    let (removed_before, freed_before) = totals();
    record_sweep(3, 400);
    let (removed_after, freed_after) = totals();
    assert!(removed_after >= removed_before + 3);
    assert!(freed_after >= freed_before + 400);
}

#[test]
fn record_sweep_of_zero_never_decreases_totals() {
    let before = totals();
    record_sweep(0, 0);
    let after = totals();
    assert!(after.0 >= before.0);
    assert!(after.1 >= before.1);
}
