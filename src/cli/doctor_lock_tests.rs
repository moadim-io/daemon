// Tests for the `global lock` check. Included into `doctor_tests.rs`, so `super::*` and its
// fixtures are in scope.

use crate::global_lock::LockStatus;
use crate::utils::time::format_local;

/// A lock status built from its raw fields, so tests need no filesystem.
fn status(
    shared: bool,
    local: bool,
    shared_since: Option<u64>,
    local_since: Option<u64>,
) -> LockStatus {
    LockStatus {
        shared,
        local,
        locked: shared || local,
        shared_since,
        local_since,
    }
}

#[test]
fn unlocked_lock_check_passes() {
    let check = lock_check(&unlocked());
    assert!(check.passed());
    assert_eq!(check.render(), "ok   global lock");
}

#[test]
fn local_lock_only_names_local_sentinel() {
    let check = lock_check(&status(false, true, None, Some(1_700_000_000)));
    assert_eq!(
        check.render(),
        format!(
            "FAIL global lock — .local.lock since {}; routines are halted, run: moadim unlock",
            format_local(1_700_000_000)
        )
    );
}

#[test]
fn shared_lock_only_names_shared_sentinel() {
    let check = lock_check(&status(true, false, Some(1_700_000_000), None));
    assert_eq!(
        check.render(),
        format!(
            "FAIL global lock — .lock since {}; routines are halted, run: moadim unlock",
            format_local(1_700_000_000)
        )
    );
}

#[test]
fn both_locks_list_both_and_use_earlier_since() {
    let check = lock_check(&status(true, true, Some(1_700_000_200), Some(1_700_000_100)));
    assert_eq!(
        check.render(),
        format!(
            "FAIL global lock — .lock and .local.lock since {}; routines are halted, run: moadim unlock",
            format_local(1_700_000_100)
        )
    );
}

#[test]
fn locked_without_since_time_omits_since() {
    let check = lock_check(&status(true, false, None, None));
    assert_eq!(
        check.render(),
        "FAIL global lock — .lock; routines are halted, run: moadim unlock"
    );
}

#[test]
fn held_lock_makes_exit_code_one() {
    let fixture = Fixture::healthy();
    let checks = run_checks(
        &fixture.path(),
        &fixture.agents(),
        true,
        &status(true, false, Some(1_700_000_000), None),
    );
    assert!(!find(&checks, "global lock").passed());
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn global_lock_check_sits_after_agents_and_before_daemon() {
    let fixture = Fixture::healthy();
    let checks = run_checks(&fixture.path(), &fixture.agents(), true, &unlocked());
    let labels: Vec<&str> = checks.iter().map(|check| check.label.as_str()).collect();
    assert_eq!(
        labels,
        ["tmux", "python3", "agent claude", "global lock", "daemon"]
    );
}
