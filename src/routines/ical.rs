//! iCalendar (RFC 5545) export of routine schedules so upcoming fire times can be
//! subscribed to in external calendars.

use chrono::{DateTime, Duration, Local, Utc};
use croner::Cron;

use super::model::{Routine, RoutineStore};

/// How far ahead (in days) the feed projects each routine's fire times.
const HORIZON_DAYS: i64 = 30;
/// Maximum events emitted per routine, bounding feed size for high-frequency schedules.
const MAX_EVENTS_PER_ROUTINE: usize = 100;
/// Product identifier advertised in the `PRODID` property.
const PRODID: &str = "-//moadim//routines//EN";

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

/// Render upcoming fire times of every enabled routine as an iCalendar (`.ics`) feed.
///
/// Each enabled routine with a parseable schedule contributes one `VEVENT` per fire time in
/// `(now, now + HORIZON_DAYS]`, capped at [`MAX_EVENTS_PER_ROUTINE`]. Fire times are evaluated in
/// the host's local timezone (matching crontab semantics) and emitted as UTC instants so the feed
/// needs no embedded `VTIMEZONE`. Disabled routines and unparseable schedules (e.g. `@reboot`)
/// contribute nothing.
pub fn build_ical(routines: &[Routine], now: DateTime<Local>) -> String {
    let dtstamp = format_utc(now.with_timezone(&Utc));
    let horizon = now + Duration::days(HORIZON_DAYS);
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        format!("PRODID:{PRODID}"),
        "CALSCALE:GREGORIAN".to_string(),
        "X-WR-CALNAME:Moadim Routines".to_string(),
    ];
    for routine in routines {
        if !routine.enabled {
            continue;
        }
        let Ok(cron) = routine.schedule.parse::<Cron>() else {
            continue;
        };
        let summary = escape_text(&routine.title);
        let description = escape_text(&format!("{} (agent: {})", routine.prompt, routine.agent));
        for fire in cron
            .iter_after(now)
            .take_while(|dt| *dt <= horizon)
            .take(MAX_EVENTS_PER_ROUTINE)
        {
            let stamp = format_utc(fire.with_timezone(&Utc));
            lines.push("BEGIN:VEVENT".to_string());
            lines.push(format!("UID:{}-{}@moadim", routine.id, stamp));
            lines.push(format!("DTSTAMP:{dtstamp}"));
            lines.push(format!("DTSTART:{stamp}"));
            lines.push(format!("SUMMARY:{summary}"));
            lines.push(format!("DESCRIPTION:{description}"));
            lines.push("END:VEVENT".to_string());
        }
    }
    lines.push("END:VCALENDAR".to_string());
    // RFC 5545 mandates CRLF line endings, including a trailing CRLF after the final line.
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out
}

/// Build the iCalendar feed for every routine currently in `store`.
pub fn svc_ical(store: &RoutineStore) -> String {
    let routines: Vec<Routine> = store.lock().unwrap().values().cloned().collect();
    build_ical(&routines, Local::now())
}

#[cfg(test)]
#[path = "ical_tests.rs"]
mod ical_tests;
