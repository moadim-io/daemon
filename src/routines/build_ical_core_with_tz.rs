#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Like [`build_ical_core`] but with the host `VTIMEZONE` info (`(TZID, UTC offset)`) passed in
/// explicitly, so tests can exercise both the `TZID`-qualified and UTC-fallback `DTSTART` forms
/// deterministically instead of depending on the test machine's own resolvable timezone.
pub(crate) fn build_ical_core_with_tz(
    routines: &[Routine],
    now: DateTime<Local>,
    cal_name: &str,
    max_events: usize,
    host_tz: Option<&(String, chrono::FixedOffset)>,
) -> String {
    let dtstamp = format_utc(now.with_timezone(&Utc));
    let horizon = now + Duration::days(HORIZON_DAYS);
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        format!("PRODID:{PRODID}"),
        "CALSCALE:GREGORIAN".to_string(),
        format!("X-WR-CALNAME:{}", escape_text(cal_name)),
        // RFC 7986 §5.7 standard hint plus the widely-honored Microsoft/Google
        // X-PUBLISHED-TTL fallback, so subscribers poll often enough to pick up
        // routine changes promptly instead of using their slow built-in default.
        format!("REFRESH-INTERVAL;VALUE=DURATION:{REFRESH_DURATION}"),
        format!("X-PUBLISHED-TTL:{REFRESH_DURATION}"),
    ];
    if let Some((tzid, offset)) = &host_tz {
        lines.extend(vtimezone_lines(tzid, *offset));
    }
    // `TZID` param-values never need quoting here: IANA zone names (e.g. `Asia/Jerusalem`) never
    // contain the `:`/`;`/`,` characters RFC 5545 §3.2 would require escaping.
    let dtstart_line = |local: DateTime<Local>, utc_stamp: &str| match &host_tz {
        Some((tzid, _)) => format!("DTSTART;TZID={tzid}:{}", format_local(local)),
        None => format!("DTSTART:{utc_stamp}"),
    };
    let globally_locked = crate::global_lock::is_globally_locked();
    for routine in routines {
        if !routine.enabled || globally_locked || routine.power_saving {
            continue;
        }
        let schedules = routine.effective_schedules();
        let summary = escape_text(&routine.title);
        let description = escape_text(&format!(
            "{} (agent: {})",
            prompt_summary(&routine.prompt),
            routine.agent
        ));
        // Fire times within the horizon, in order. Kept as a stateful iterator so that after the
        // per-routine cap is spent we can peek whether more fires remain inside the horizon and, if
        // so, surface the truncation rather than letting the feed silently stop short of 30 days.
        //
        // `snoozed_until`/`skip_runs` are mutually exclusive (enforced by `svc_snooze`): a fire is
        // dropped either because it falls before the snooze deadline, or because it's among the
        // next `skip_runs` fires that `svc_trigger_scheduled` will skip and decrement past. Both
        // mirror the exact skip performed there so the feed matches what will actually run.
        let snoozed_until = routine.snoozed_until;
        let skip_runs = routine.skip_runs.unwrap_or(0) as usize;
        let mut all_fires: Vec<DateTime<Local>> = schedules
            .iter()
            .filter_map(|schedule| schedule.parse::<Cron>().ok())
            .flat_map(|cron| cron.iter_after(now).take_while(|dt| *dt <= horizon))
            .collect();
        all_fires.sort();
        all_fires.dedup();
        let mut fires = all_fires
            .into_iter()
            .filter(move |dt| {
                snoozed_until
                    .is_none_or(|until| u64::try_from(dt.timestamp()).is_ok_and(|ts| ts >= until))
            })
            .skip(skip_runs);
        let mut emitted = 0_usize;
        for fire in fires.by_ref().take(max_events) {
            let stamp = format_utc(fire.with_timezone(&Utc));
            lines.push("BEGIN:VEVENT".to_string());
            lines.push(format!("UID:{}-{}@moadim", routine.id, stamp));
            lines.push(format!("DTSTAMP:{dtstamp}"));
            lines.push(dtstart_line(fire, &stamp));
            lines.push(format!("DURATION:{EVENT_DURATION}"));
            // The feed is purely informational ("when will my loops fire?"), so a
            // fire must not consume the subscriber's free/busy time. RFC 5545
            // §3.8.2.7 defaults `TRANSP` to `OPAQUE` (counts as busy); mark each
            // event `TRANSPARENT` so it never blocks availability. The legacy
            // `X-MICROSOFT-CDO-BUSYSTATUS:FREE` carries the same intent to Outlook
            // clients that honor the Microsoft property instead of `TRANSP`.
            lines.push("TRANSP:TRANSPARENT".to_string());
            lines.push("X-MICROSOFT-CDO-BUSYSTATUS:FREE".to_string());
            lines.push(format!("SUMMARY:{summary}"));
            lines.push(format!("DESCRIPTION:{description}"));
            lines.push("END:VEVENT".to_string());
            emitted += 1;
        }
        // Cap reached with fires still pending inside the horizon: append a marker VEVENT at the
        // first omitted fire so subscribers see the projection was truncated and where it stops.
        if emitted == max_events {
            if let Some(next) = fires.next() {
                let stamp = format_utc(next.with_timezone(&Utc));
                let note = escape_text(&format!(
                    "{}: schedule truncated — only the first {} of more upcoming runs through {} \
                     are listed. Subscribe to the daemon directly for the full schedule.",
                    routine.title,
                    max_events,
                    horizon.format("%Y-%m-%d")
                ));
                lines.push("BEGIN:VEVENT".to_string());
                lines.push(format!("UID:{}-truncated@moadim", routine.id));
                lines.push(format!("DTSTAMP:{dtstamp}"));
                lines.push(dtstart_line(next, &stamp));
                // Mirror the regular fire VEVENT's DURATION/TRANSP/BUSYSTATUS (see the comments
                // on EVENT_DURATION and on the regular-fire VEVENT above): without a DURATION
                // this marker is a zero-length instant, which most calendar UIs render as an
                // invisible sliver — defeating its one job of telling subscribers the feed was
                // truncated.
                lines.push(format!("DURATION:{EVENT_DURATION}"));
                lines.push("TRANSP:TRANSPARENT".to_string());
                lines.push("X-MICROSOFT-CDO-BUSYSTATUS:FREE".to_string());
                lines.push(format!("SUMMARY:⚠ {summary} (schedule truncated)"));
                lines.push(format!("DESCRIPTION:{note}"));
                lines.push("END:VEVENT".to_string());
            }
        }
    }
    lines.push("END:VCALENDAR".to_string());
    // RFC 5545 mandates CRLF line endings, including a trailing CRLF after the final
    // line. Each content line is folded (§3.1) so no physical line exceeds 75 octets.
    let mut out = lines
        .iter()
        .map(|line| fold_line(line))
        .collect::<Vec<_>>()
        .join("\r\n");
    out.push_str("\r\n");
    out
}

/// Test-only entry point: build the iCalendar feed with a custom per-routine event cap so tests
/// can exercise the truncation-marker path without needing a cron schedule that fires exactly
/// [`MAX_EVENTS_PER_ROUTINE`] times in the 30-day horizon.
#[cfg(test)]
pub(crate) fn build_ical_with_cap(
    routines: &[Routine],
    now: DateTime<Local>,
    max_events: usize,
) -> String {
    build_ical_core(routines, now, DEFAULT_CAL_NAME, max_events)
}

pub(crate) fn build_ical_with_tz(
    routines: &[Routine],
    now: DateTime<Local>,
    max_events: usize,
    host_tz: Option<(String, chrono::FixedOffset)>,
) -> String {
    build_ical_core_with_tz(
        routines,
        now,
        DEFAULT_CAL_NAME,
        max_events,
        host_tz.as_ref(),
    )
}

/// Build the iCalendar feed for every routine currently in `store`.
///
/// Refreshes the store from `dir` first so the feed reflects routines pulled or edited on disk under
/// a running daemon without a restart (disk is the source of truth).
pub fn svc_ical(store: &RoutineStore, dir: &std::path::Path) -> String {
    crate::routine_storage::reload_store_from_dir(store, dir);
    let routines: Vec<Routine> = store.lock_recover().values().cloned().collect();
    build_ical(&routines, Local::now())
}

/// Build the iCalendar feed for a single routine by `id` (issue #263).
///
/// The calendar is named after the routine so a subscribed feed reads as that routine
/// rather than the generic all-routines name. An unknown id yields a well-formed empty
/// calendar (named [`DEFAULT_CAL_NAME`]) rather than an error, mirroring how a disabled
/// routine already contributes no events.
///
/// Refreshes the store from `dir` first so the feed reflects a routine pulled or edited on disk
/// under a running daemon without a restart (disk is the source of truth).
pub fn svc_ical_routine(store: &RoutineStore, dir: &std::path::Path, id: &str) -> String {
    crate::routine_storage::reload_store_from_dir(store, dir);
    let routine = store.lock_recover().get(id).cloned();
    match routine {
        Some(routine) => {
            let cal_name = routine.title.clone();
            build_ical_named(std::slice::from_ref(&routine), Local::now(), &cal_name)
        }
        None => build_ical_named(&[], Local::now(), DEFAULT_CAL_NAME),
    }
}
