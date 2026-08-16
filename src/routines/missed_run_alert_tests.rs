use super::*;

fn routine(overrides: impl FnOnce(&mut Routine)) -> Routine {
    let mut routine = Routine {
        id: "rid".into(),
        schedule: "*/5 * * * *".into(),
        schedules: vec![],
        title: "Morning review".into(),
        agent: "claude".into(),
        model: None,
        prompt: "review".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        disabled_reason: None,
        source: "managed".into(),
        created_at: 100,
        updated_at: 100,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        consecutive_failures: 0,
        auto_disabled_reason: None,
        ttl_secs: None,
        max_runtime_secs: None,
        failure_threshold: None,
        notifications: Default::default(),
        tags: vec![],
        env: std::collections::HashMap::new(),
        timezone: None,
    };
    overrides(&mut routine);
    routine
}

#[test]
fn reports_latest_elapsed_fire_after_last_scheduled_trigger() {
    // `*/5` is offset-invariant: every local timezone fires on epoch seconds divisible by 300.
    let routine = routine(|routine| routine.last_scheduled_trigger_at = Some(1_000));

    assert_eq!(
        missed_scheduled_run_at_now(&routine, &routine.effective_schedules(), 1_700),
        Some(1_500)
    );
}

#[test]
fn waits_one_grace_minute_before_alerting_current_fire() {
    let routine = routine(|routine| routine.last_scheduled_trigger_at = Some(1_200));

    assert_eq!(
        missed_scheduled_run_at_now(&routine, &routine.effective_schedules(), 1_530),
        None
    );
}

#[test]
fn does_not_alert_after_the_fire_was_recorded() {
    let routine = routine(|routine| routine.last_scheduled_trigger_at = Some(1_500));

    assert_eq!(
        missed_scheduled_run_at_now(&routine, &routine.effective_schedules(), 1_700),
        None
    );
}

#[test]
fn ignores_intentionally_suppressed_or_unassigned_routines() {
    let disabled = routine(|routine| routine.enabled = false);
    assert_eq!(
        missed_scheduled_run_at_now(&disabled, &disabled.effective_schedules(), 1_782_196_520),
        None
    );

    let snoozed = routine(|routine| routine.snoozed_until = Some(1_782_200_000));
    assert_eq!(
        missed_scheduled_run_at_now(&snoozed, &snoozed.effective_schedules(), 1_782_196_520),
        None
    );

    let skip_runs = routine(|routine| routine.skip_runs = Some(1));
    assert_eq!(
        missed_scheduled_run_at_now(&skip_runs, &skip_runs.effective_schedules(), 1_782_196_520),
        None
    );

    let unassigned = routine(|routine| routine.machines.clear());
    assert_eq!(
        missed_scheduled_run_at_now(
            &unassigned,
            &unassigned.effective_schedules(),
            1_782_196_520
        ),
        None
    );
}

#[test]
fn wrapper_reports_missed_fire() {
    let routine = routine(|routine| routine.last_scheduled_trigger_at = Some(1_000));
    let _ = missed_scheduled_run_at(&routine, &routine.effective_schedules());
}

#[test]
fn underflowing_now_does_not_alert() {
    let routine = routine(|_| {});
    assert_eq!(
        missed_scheduled_run_at_now(&routine, &routine.effective_schedules(), 30),
        None
    );
}

#[test]
fn baseline_outside_supported_i64_range_does_not_alert() {
    let routine = routine(|routine| routine.created_at = u64::MAX);
    assert_eq!(
        missed_scheduled_run_at_now(&routine, &routine.effective_schedules(), u64::MAX),
        None
    );
}

#[test]
fn timestamp_outside_supported_range_does_not_alert() {
    let baseline_overflow = routine(|routine| routine.created_at = u64::MAX - 100);
    assert_eq!(
        missed_scheduled_run_at_now(
            &baseline_overflow,
            &baseline_overflow.effective_schedules(),
            u64::MAX,
        ),
        None
    );

    let latest_overflow = routine(|routine| routine.created_at = 0);
    assert_eq!(
        missed_scheduled_run_at_now(
            &latest_overflow,
            &latest_overflow.effective_schedules(),
            u64::MAX,
        ),
        None
    );
}
