//! iCalendar (RFC 5545) export of routine schedules so upcoming fire times can be
//! subscribed to in external calendars.

use crate::utils::lock::LockRecover;
use chrono::{DateTime, Duration, Local, Utc};
use croner::Cron;

use super::model::{Routine, RoutineStore};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "ical_part2.rs"]
mod ical_part2;
pub(crate) use ical_part2::*;

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
include!("ical_part3.rs");
