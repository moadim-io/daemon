//! Translate a moadim/cron schedule into Windows Task Scheduler (`schtasks`) trigger flags.
//!
//! Cron is strictly more expressive than the Task Scheduler's calendar triggers, so this covers the
//! common subset that maps cleanly (every-N-minutes, hourly, daily-at, weekly-on-days,
//! monthly-on-day, and the `@keyword` shorthands). Anything outside that subset returns `None`; the
//! caller logs a warning and leaves the entry unscheduled rather than registering a wrong time.
//!
//! Compiled on Windows (where it is used) and under `cfg(test)` (so the pure translation can be
//! unit-tested on any host).

/// Map a cron weekday number (`0`/`7` = Sunday … `6` = Saturday) to the `schtasks /D` token.
fn weekday_token(num: u8) -> Option<&'static str> {
    match num {
        0 | 7 => Some("SUN"),
        1 => Some("MON"),
        2 => Some("TUE"),
        3 => Some("WED"),
        4 => Some("THU"),
        5 => Some("FRI"),
        6 => Some("SAT"),
        _ => None,
    }
}

/// Parse a field as a single non-negative integer in `[min, max]`, else `None`.
fn single_int(field: &str, min: u8, max: u8) -> Option<u8> {
    let value: u8 = field.parse().ok()?;
    (value >= min && value <= max).then_some(value)
}

/// Parse a minute/hour `*/n` step field into `n`, else `None`.
fn step(field: &str) -> Option<u8> {
    let rest = field.strip_prefix("*/")?;
    let num: u8 = rest.parse().ok()?;
    (num >= 1).then_some(num)
}

/// Expand a day-of-week field (`1-5`, `1,3,5`, `MON`, `*`) into `schtasks /D` tokens.
///
/// Returns `None` for `*` (caller treats that as "every day" → a daily trigger) or anything
/// unparseable.
fn weekday_tokens(field: &str) -> Option<Vec<&'static str>> {
    if field == "*" {
        return None;
    }
    let mut tokens: Vec<&'static str> = Vec::new();
    for part in field.split(',') {
        if let Some((lo, hi)) = part.split_once('-') {
            let lo: u8 = lo.parse().ok()?;
            let hi: u8 = hi.parse().ok()?;
            if lo > hi {
                return None;
            }
            for num in lo..=hi {
                tokens.push(weekday_token(num)?);
            }
        } else {
            tokens.push(weekday_token(part.parse().ok()?)?);
        }
    }
    // `field` is a non-empty cron token, so `split(',')` always yields at least one part and the
    // loop pushes at least one token (or returns `None`); `tokens` is therefore never empty here.
    Some(tokens)
}

/// Format `hour:minute` as the zero-padded 24-hour `HH:MM` that `schtasks /ST` expects.
fn start_time(hour: u8, minute: u8) -> String {
    format!("{hour:02}:{minute:02}")
}

/// Translate `schedule` into the `schtasks /Create` trigger flags (everything after `/TN … /TR …`),
/// or `None` when the schedule has no faithful Task Scheduler equivalent.
pub fn cron_to_schtasks(schedule: &str) -> Option<Vec<String>> {
    let trimmed = schedule.trim();

    if let Some(rest) = trimmed.strip_prefix('@') {
        return keyword_trigger(rest);
    }

    let fields: Vec<&str> = trimmed.split_ascii_whitespace().collect();
    // Accept moadim's 5-field OS form; a 7-field form is reduced to its middle five.
    let five: Vec<&str> = match fields.len() {
        5 => fields,
        7 => fields[1..6].to_vec(),
        _ => return None,
    };
    let (minute, hour, dom, month, dow) = (five[0], five[1], five[2], five[3], five[4]);

    // Month restrictions are not representable alongside the day triggers below; bail rather than
    // silently dropping the constraint.
    if month != "*" {
        return None;
    }

    // Every N minutes: `*/n * * * *`.
    if let Some(num) = step(minute) {
        if hour == "*" && dom == "*" && dow == "*" {
            return Some(flags(["/SC", "MINUTE", "/MO", &num.to_string()]));
        }
        return None;
    }

    let minute = single_int(minute, 0, 59)?;

    // Every hour at a fixed minute: `m * * * *`.
    if hour == "*" {
        if dom == "*" && dow == "*" {
            return Some(flags([
                "/SC",
                "HOURLY",
                "/MO",
                "1",
                "/ST",
                &start_time(0, minute),
            ]));
        }
        return None;
    }

    let hour = single_int(hour, 0, 23)?;
    let time = start_time(hour, minute);

    // Weekly on specific weekdays: `m h * * dow`.
    if dom == "*" {
        if let Some(days) = weekday_tokens(dow) {
            return Some(flags([
                "/SC",
                "WEEKLY",
                "/D",
                &days.join(","),
                "/ST",
                &time,
            ]));
        }
        // dow == "*" → falls through to daily.
        if dow == "*" {
            return Some(flags(["/SC", "DAILY", "/ST", &time]));
        }
        return None;
    }

    // Monthly on a day-of-month: `m h dom * *`.
    if dow == "*" {
        let day = single_int(dom, 1, 31)?;
        return Some(flags([
            "/SC",
            "MONTHLY",
            "/D",
            &day.to_string(),
            "/ST",
            &time,
        ]));
    }

    // Both day-of-month and day-of-week constrained: no clean Task Scheduler equivalent.
    None
}

/// Trigger flags for the `@keyword` shorthands.
fn keyword_trigger(keyword: &str) -> Option<Vec<String>> {
    match keyword {
        "reboot" => Some(flags(["/SC", "ONSTART"])),
        "hourly" => Some(flags(["/SC", "HOURLY", "/MO", "1", "/ST", "00:00"])),
        "daily" | "midnight" => Some(flags(["/SC", "DAILY", "/ST", "00:00"])),
        "weekly" => Some(flags(["/SC", "WEEKLY", "/D", "SUN", "/ST", "00:00"])),
        "monthly" => Some(flags(["/SC", "MONTHLY", "/D", "1", "/ST", "00:00"])),
        "yearly" | "annually" => Some(flags([
            "/SC", "MONTHLY", "/M", "JAN", "/D", "1", "/ST", "00:00",
        ])),
        _ => None,
    }
}

/// Collect a flag list into owned `String`s.
fn flags<const N: usize>(parts: [&str; N]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod schedule_tests;
