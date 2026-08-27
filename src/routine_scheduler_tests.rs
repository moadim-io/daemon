#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use chrono::{Duration, Local, Timelike};

use super::due_routine_ids;

#[test]
fn due_routine_ids_selects_enabled_local_machine_routines_in_recent_window() {
    let now = Local::now()
        .with_second(30)
        .and_then(|time| time.with_nanosecond(0))
        .expect("valid local timestamp");
    let mut due = crate::test_fixtures::routine_fixture("due", "Due").build();
    due.schedule = "* * * * *".into();
    let mut disabled = crate::test_fixtures::routine_fixture("disabled", "Disabled")
        .enabled(false)
        .build();
    disabled.schedule = "* * * * *".into();
    let mut other_machine = crate::test_fixtures::routine_fixture("other", "Other").build();
    other_machine.schedule = "* * * * *".into();
    other_machine.machines = vec!["another-machine".into()];
    let mut not_due = crate::test_fixtures::routine_fixture("not-due", "Not due").build();
    not_due.schedule = "0 0 1 1 *".into();

    let due_ids = due_routine_ids(
        &[due, disabled, other_machine, not_due],
        now - Duration::seconds(90),
        now,
        &crate::machine::current_machine(),
    );

    assert_eq!(due_ids, vec!["due"]);
}

#[test]
fn due_routine_ids_deduplicates_overlapping_schedules_for_one_routine() {
    let now = Local::now()
        .with_second(30)
        .and_then(|time| time.with_nanosecond(0))
        .expect("valid local timestamp");
    let mut routine = crate::test_fixtures::routine_fixture("once", "Once").build();
    routine.schedules = vec!["* * * * *".into(), "* * * * *".into()];

    let due_ids = due_routine_ids(
        &[routine],
        now - Duration::seconds(90),
        now,
        &crate::machine::current_machine(),
    );

    assert_eq!(due_ids, vec!["once"]);
}
