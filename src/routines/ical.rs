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
/// the host's local timezone (matching crontab semantics) and emitted as UTC instants so the feed
/// needs no embedded `VTIMEZONE`. Disabled routines and unparseable schedules (e.g. `@reboot`)
/// contribute nothing. The calendar is named [`DEFAULT_CAL_NAME`]; for a single-routine feed see
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
fn build_ical_core(
    routines: &[Routine],
    now: DateTime<Local>,
    cal_name: &str,
    max_events: usize,
) -> String {
    let dtstamp = format_utc(now.with_timezone(&Utc));
    let horizon = now + Duration::days(HORIZON_DAYS);
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        format!("PRODID:{PRODID}"),
        "CALSCALE:GREGORIAN".to_string(),
        format!("X-WR-CALNAME:{}", escape_text(cal_name)),
    ];
    let globally_locked = crate::global_lock::is_globally_locked();
    for routine in routines {
        if !routine.enabled || globally_locked {
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
        let mut fires = cron.iter_after(now).take_while(|dt| *dt <= horizon);
        let mut emitted = 0usize;
        for fire in fires.by_ref().take(max_events) {
            let stamp = format_utc(fire.with_timezone(&Utc));
            lines.push("BEGIN:VEVENT".to_string());
            lines.push(format!("UID:{}-{}@moadim", routine.id, stamp));
            lines.push(format!("DTSTAMP:{dtstamp}"));
            lines.push(format!("DTSTART:{stamp}"));
            lines.push(format!("DURATION:{EVENT_DURATION}"));
            lines.push(format!("SUMMARY:{summary}"));
            lines.push(format!("DESCRIPTION:{description}"));
            // A fire time is a momentary trigger, not a block of busy time. Mark
            // the event TRANSPARENT (RFC 5545 §3.8.2.7) so subscribing to the feed
            // does not show the operator as BUSY at every scheduled run.
            lines.push("TRANSP:TRANSPARENT".to_string());
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
                lines.push(format!("DTSTART:{stamp}"));
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

/// Build the iCalendar feed for every routine currently in `store`.
pub fn svc_ical(store: &RoutineStore) -> String {
    let routines: Vec<Routine> = store.lock_recover().values().cloned().collect();
    build_ical(&routines, Local::now())
}

/// Build the iCalendar feed for a single routine by `id` (issue #263).
///
/// The calendar is named after the routine so a subscribed feed reads as that routine
/// rather than the generic all-routines name. An unknown id yields a well-formed empty
/// calendar (named [`DEFAULT_CAL_NAME`]) rather than an error, mirroring how a disabled
/// routine already contributes no events.
pub fn svc_ical_routine(store: &RoutineStore, id: &str) -> String {
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
