#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_trigger_skips_spawn_when_the_global_concurrency_cap_is_reached() {
    // The global cap (#335) must trip even when the live sessions belong to *other* routines —
    // unlike the per-routine overlap guard, it counts every `moadim-`-prefixed session regardless
    // of slug. One live (unrelated) session already meets a cap of 1, so the fire must be skipped.
    assert!(trigger_under_concurrency_cap("zzz", 1, Some("1")));
}

#[test]
fn svc_trigger_does_not_cap_when_max_concurrent_runs_is_unset_or_zero() {
    // `0`/unset now means unbounded (#335 policy flip) — a live session count that would have
    // tripped the old hardcoded default-of-4 cap must not be skipped when the cap is unset.
    assert!(!trigger_under_concurrency_cap("unlimited", 4, None));
}
