#[test]
fn pinned_routine_fires_are_evaluated_in_its_own_timezone() {
    // 09:00 in Tokyo (UTC+9, no DST) is 00:00 UTC, whatever zone the host runs in.
    let mut routine = routine_with("tokyo", "0 9 * * *", true);
    routine.timezone = Some("Asia/Tokyo".to_string());
    let feed = build_ical_with_tz(&[routine], fixed_now(), 100, None);
    let starts: Vec<&str> = feed.lines().filter(|line| line.starts_with("DTSTART:")).collect();
    assert!(starts.len() >= 30, "{feed}");
    assert!(starts.iter().all(|line| line.ends_with("T000000Z")), "{starts:?}");
}
