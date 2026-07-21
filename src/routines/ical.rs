//! iCalendar (RFC 5545) export of routine schedules so upcoming fire times can be
//! subscribed to in external calendars.

use crate::utils::lock::LockRecover;
use chrono::{DateTime, Duration, Local, Utc};
use croner::Cron;

use super::model::{Routine, RoutineStore};

/// How far ahead (in days) the feed projects each routine's fire times.
const HORIZON_DAYS: i64 = 30;
/// Maximum events emitted per routine, bounding feed size for high-frequency schedules.
const MAX_EVENTS_PER_ROUTINE: usize = 100;
/// Product identifier advertised in the `PRODID` property.
const PRODID: &str = "-//moadim//routines//EN";
/// Suggested polling interval advertised to subscribers, as an iCalendar DURATION.
///
/// Routine schedules can change at any time, but the feed itself is regenerated on
/// every request, so the only freshness limit is how often a subscriber re-fetches.
/// Without a hint, clients fall back to their own default (often 12–24h), making
/// routine edits lag for hours. One hour balances freshness against feed load.
const REFRESH_DURATION: &str = "PT1H";
/// Duration assigned to each fire so it renders as a visible block rather than a
/// zero-length instant. RFC 5545 requires a `VEVENT` to carry either `DTEND` or
/// `DURATION`; a routine fire has no intrinsic end, so a short fixed window is used.
const EVENT_DURATION: &str = "PT15M";
/// Calendar display name (`X-WR-CALNAME`) for the unfiltered, all-routines feed.
const DEFAULT_CAL_NAME: &str = "Moadim Routines";

/// Escape a text value for an iCalendar property per RFC 5545 §3.3.11.
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            // RFC 5545 §3.3.11: a TEXT value cannot contain a raw carriage
            // return. Normalize both CRLF and a lone CR to the same escaped
            // newline as a bare LF, so no stray '\r' ever reaches the feed.
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\\n");
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Format a UTC instant as an iCalendar UTC date-time (`YYYYMMDDTHHMMSSZ`).
fn format_utc(dt: DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Format a local wall-clock instant as an iCalendar local date-time (`YYYYMMDDTHHMMSS`, no
/// trailing `Z`), for use with a `TZID`-qualified `DTSTART` (RFC 5545 §3.3.5).
fn format_local(dt: DateTime<Local>) -> String {
    dt.format("%Y%m%dT%H%M%S").to_string()
}

/// Format a UTC offset as an RFC 5545 §3.3.14 `utc-offset` (`+HHMM`, or `+HHMMSS` when the
/// offset carries seconds — some pre-1900 zones do).
fn format_utc_offset(offset: chrono::FixedOffset) -> String {
    let total = offset.local_minus_utc();
    let sign = if total < 0 { '-' } else { '+' };
    let total = total.unsigned_abs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if seconds == 0 {
        format!("{sign}{hours:02}{minutes:02}")
    } else {
        format!("{sign}{hours:02}{minutes:02}{seconds:02}")
    }
}

/// Build the `VTIMEZONE` component lines identifying the host's local zone.
///
/// Emits a single `STANDARD` sub-component pinned to the feed's current UTC offset rather than a
/// full `STANDARD`/`DAYLIGHT` pair with DST transition rules — the daemon has no timezone-database
/// dependency to derive those from. A routine whose zone observes DST may therefore display
/// shifted by the DST delta in a subscriber's calendar once the host crosses a transition after the
/// feed was generated. This still fixes the common complaint (issue #387): a subscriber whose
/// calendar defaults to a *different* zone than the host now sees the routine's actual configured
/// local time instead of the UTC instant reinterpreted in their own zone.
fn vtimezone_lines(tzid: &str, offset: chrono::FixedOffset) -> Vec<String> {
    let offset_str = format_utc_offset(offset);
    vec![
        "BEGIN:VTIMEZONE".to_string(),
        format!("TZID:{}", escape_text(tzid)),
        "BEGIN:STANDARD".to_string(),
        "DTSTART:19700101T000000".to_string(),
        format!("TZOFFSETFROM:{offset_str}"),
        format!("TZOFFSETTO:{offset_str}"),
        "END:STANDARD".to_string(),
        "END:VTIMEZONE".to_string(),
    ]
}

/// Maximum characters of a routine prompt shown in a `DESCRIPTION` before truncation.
const DESCRIPTION_PROMPT_MAX: usize = 120;

/// Build a compact, single-line summary of a routine prompt for a `VEVENT` `DESCRIPTION`.
///
/// Prompts are routinely multi-KB and identical across all of a routine's fire times, so embedding
/// the full prompt in every event bloats the feed and makes calendar entries unreadable. Take the
/// first non-empty line, trimmed and truncated to [`DESCRIPTION_PROMPT_MAX`] characters, appending
/// an ellipsis when any content (a longer line, or further lines) was dropped.
fn prompt_summary(prompt: &str) -> String {
    let non_empty = || prompt.lines().filter(|line| !line.trim().is_empty());
    let first_line = non_empty().next().unwrap_or("").trim();
    let has_more_lines = non_empty().count() > 1;
    let truncated: String = first_line.chars().take(DESCRIPTION_PROMPT_MAX).collect();
    if has_more_lines || truncated.chars().count() < first_line.chars().count() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

/// Maximum octets per physical content line per RFC 5545 §3.1 (excluding CRLF).
const FOLD_LIMIT: usize = 75;

/// Fold a content line per RFC 5545 §3.1 so no physical line exceeds
/// [`FOLD_LIMIT`] octets (excluding the CRLF terminator).
///
/// Continuation lines are introduced with `CRLF` followed by a single leading
/// space, and that space counts toward the octet limit. Folding measures **octets**
/// (UTF-8 byte length) but only ever breaks on character boundaries, so a multibyte
/// character is never split across a fold.
fn fold_line(line: &str) -> String {
    if line.len() <= FOLD_LIMIT {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + line.len() / FOLD_LIMIT + 1);
    // First physical line gets the full budget; each continuation spends one octet
    // on its leading space.
    let mut budget = FOLD_LIMIT;
    for ch in line.chars() {
        let char_len = ch.len_utf8();
        if char_len > budget {
            out.push_str("\r\n ");
            budget = FOLD_LIMIT - 1;
        }
        out.push(ch);
        budget -= char_len;
    }
    out
}

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

/// Like [`build_ical_core`] but with the host `VTIMEZONE` info (`(TZID, UTC offset)`) passed in
/// explicitly, so tests can exercise both the `TZID`-qualified and UTC-fallback `DTSTART` forms
/// deterministically instead of depending on the test machine's own resolvable timezone.
fn build_ical_core_with_tz(
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
        let Ok(cron) = routine.schedule.parse::<Cron>() else {
            continue;
        };
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
        let mut fires = cron
            .iter_after(now)
            .take_while(|dt| *dt <= horizon)
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

#[cfg(test)]
#[path = "ical_tests.rs"]
mod ical_tests;
