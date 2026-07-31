
/// Render upcoming fire times of every enabled routine as an iCalendar (`.ics`) feed.
///
/// Each enabled routine with a parseable schedule contributes one `VEVENT` per fire time in
/// `(now, now + HORIZON_DAYS]`, capped at [`MAX_EVENTS_PER_ROUTINE`]. Fire times are evaluated in
/// the host's local timezone (matching crontab semantics). When that zone can be named (see
/// [`super::model::local_timezone`]), each `DTSTART` is `TZID`-qualified with the local wall-clock
/// time, backed by one `VTIMEZONE` component (issue #387) — so a subscriber whose calendar
/// defaults to a different zone still sees the routine's actual configured local time. When the
/// zone can't be resolved, the feed falls back to a bare UTC-instant `DTSTART` with no
/// `VTIMEZONE`, exactly as before. Disabled, power-saving, and unparseable-schedule (e.g. `@reboot`)
/// routines contribute nothing. A snoozed routine (`snoozed_until` in the future, or `skip_runs`
/// above zero) has its would-be-skipped fires filtered out too, mirroring the skip that
/// `svc_trigger_scheduled` actually performs at fire time, so the feed never advertises a run that
/// will silently no-op. The calendar is named [`DEFAULT_CAL_NAME`]; for a single-routine feed see
/// [`build_ical_named`].
///
/// When a routine fires more often than the cap allows within the horizon, the count cap is hit
/// before the horizon is exhausted. To keep that truncation from silently reading as "covered the
/// whole 30 days", a trailing marker `VEVENT` (UID `…-truncated@moadim`) is appended at the first
/// omitted fire time, telling subscribers the feed was capped and where the projection stops.
pub fn build_ical(routines: &[Routine], now: DateTime<Local>) -> String {
    build_ical_named(routines, now, DEFAULT_CAL_NAME)
}

/// Like [`build_ical`] but with an explicit `X-WR-CALNAME`.
///
/// Used by the per-routine feed (`GET /routines.ics?routine=<id>`, issue #263) so a subscribed
/// calendar is named after the routine instead of the generic [`DEFAULT_CAL_NAME`]. The name is
/// escaped per RFC 5545 like any other text value.
fn build_ical_named(routines: &[Routine], now: DateTime<Local>, cal_name: &str) -> String {
    build_ical_core(routines, now, cal_name, MAX_EVENTS_PER_ROUTINE)
}

/// Core iCalendar builder parameterised by `max_events` so tests can exercise the truncation paths
/// with a small cap without needing a schedule that fires exactly [`MAX_EVENTS_PER_ROUTINE`] times.
///
/// Resolves the host's `VTIMEZONE` info itself (see [`build_ical_core_with_tz`] for the
/// test-only seam that overrides it).
fn build_ical_core(
    routines: &[Routine],
    now: DateTime<Local>,
    cal_name: &str,
    max_events: usize,
) -> String {
    // `None` when the host zone can't be named, in which case every DTSTART falls back to the
    // original bare UTC-instant form (see `vtimezone_lines`'s doc comment for the scope of what a
    // `Some` here does and doesn't model).
    let host_tz = super::model::local_timezone().map(|tzid| (tzid, *now.offset()));
    build_ical_core_with_tz(routines, now, cal_name, max_events, host_tz.as_ref())
}

/// Test-only entry point: build the iCalendar feed with an explicit per-routine event cap *and*
/// an explicit host `VTIMEZONE` override (`(TZID, UTC offset)`, or `None` for the
/// no-resolvable-zone fallback), so both the truncation-marker path and the choice between a
/// `TZID`-qualified and a bare-UTC `DTSTART` can be tested deterministically instead of depending
/// on whichever timezone the test machine itself resolves to.
#[cfg(test)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "owned Option reads cleanest at test call sites; internally borrowed once"
)]
#[cfg(test)]
#[path = "ical_tests.rs"]
mod ical_tests;

#[cfg(test)]
#[path = "ical_offset_tests.rs"]
mod ical_offset_tests;
