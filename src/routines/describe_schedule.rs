/// Render a human-readable schedule description for `schedule`, appending the
/// timezone in parentheses when known. Returns `None` when the cron expression
/// cannot be parsed.
pub(crate) fn describe_schedule(schedule: &str, timezone: Option<&str>) -> Option<String> {
    schedule.parse::<Cron>().ok().map(|cron| {
        let desc = cron.describe();
        match timezone {
            Some(tz) => format!("{desc} ({tz})"),
            None => desc,
        }
    })
}
