
#[test]
fn effective_max_runtime_uses_explicit_value() {
    let mut routine = routine_with("@daily", None);
    routine.max_runtime_secs = Some(1234);
    assert_eq!(routine.effective_max_runtime_secs(), 1234);
    // An explicit value above the cap is clamped down to it.
    routine.max_runtime_secs = Some(u64::MAX);
    assert_eq!(routine.effective_max_runtime_secs(), MAX_RUNTIME_SECS);
}

#[test]
fn effective_ttl_falls_back_to_cap_when_schedule_never_fires() {
    // "Feb 30" parses as a valid cron expression but matches no real date, so the
    // schedule yields no future fire times. `cron_interval_secs` returns None at the
    // first `fires.next()?`, and `effective_ttl_secs` falls back to the cap.
    assert_eq!(
        routine_with("0 0 30 2 *", None).effective_ttl_secs(),
        MAX_TTL_SECS
    );
    // An explicit ttl below the cap still wins even when the interval can't be computed.
    assert_eq!(
        routine_with("0 0 30 2 *", Some(15)).effective_ttl_secs(),
        15
    );
}

#[test]
fn parse_workbench_name_overflowing_timestamp_returns_none() {
    // All-digit suffix that is too large to fit in u64 (20 nines > u64::MAX):
    // the digit-only guard passes but `ts.parse::<u64>().ok()` returns None → function returns None.
    assert!(parse_workbench_name("slug-99999999999999999999").is_none());
}

#[test]
fn cron_interval_secs_supports_at_daily() {
    assert_eq!(super::ttl::cron_interval_secs("@daily"), Some(24 * 60 * 60));
}

#[test]
fn cron_interval_secs_is_stable_regardless_of_the_current_time() {
    // Fires at 09:00 and 09:30 daily: the true minimum gap is always 30 minutes, but "the next two
    // fires from now" alone gives 30 minutes when `now` is just before 09:00 and ~23.5 hours when
    // `now` is just after 09:00 — sampling only two fires makes the result flip depending on
    // wall-clock time. Run at any real `now`, this must always resolve to the 30-minute minimum.
    assert_eq!(
        super::ttl::cron_interval_secs("0,30 9 * * *"),
        Some(30 * 60)
    );
}
