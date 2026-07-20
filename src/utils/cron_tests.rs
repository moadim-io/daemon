use super::*;

#[test]
fn validate_cron_accepts_valid() {
    assert!(validate_cron("30 9 * * 1-5").is_ok());
    assert!(validate_cron("@daily").is_ok());
    // legacy 7-field is still accepted
    assert!(validate_cron("0 30 9 * * 1-5 *").is_ok());
}

#[test]
fn validate_cron_accepts_5_field() {
    assert!(validate_cron("0 9 * * *").is_ok());
    assert!(validate_cron("30 9 * * 1-5").is_ok());
    assert!(validate_cron("*/15 * * * *").is_ok());
}

#[test]
fn validate_cron_rejects_invalid() {
    assert!(validate_cron("not a cron").is_err());
    assert!(validate_cron("99 99 99 99 99").is_err());
}

#[test]
fn validate_cron_accepts_6_field() {
    // croner accepts a leading seconds field; we must too.
    assert!(validate_cron("0 */5 * * * *").is_ok());
    assert!(validate_cron("30 0 9 * * 1-5").is_ok());
    // croner accepts 6-field `sec min hour dom month dow`; validate_cron
    // normalizes it down before parsing, so it must be accepted.
    assert!(validate_cron("*/30 * * * * *").is_ok());
}

#[test]
fn normalize_schedule_strips_seconds_from_6_field() {
    // 6-field `sec min hour dom month dow` -> 5-field `min hour dom month dow`.
    // Without this, the 6-field string lands in the OS crontab verbatim and
    // the routine silently never fires.
    assert_eq!(normalize_schedule("0 */5 * * * *"), "*/5 * * * *");
    assert_eq!(normalize_schedule("30 0 9 * * 1-5"), "0 9 * * 1-5");
}

#[test]
fn normalize_schedule_strips_seconds_and_year_from_7_field() {
    assert_eq!(normalize_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
}

#[test]
fn normalize_schedule_passes_through_5_field_and_keywords() {
    assert_eq!(normalize_schedule("*/15 * * * *"), "*/15 * * * *");
    assert_eq!(normalize_schedule("@daily"), "@daily");
}

#[test]
fn normalize_schedule_projects_to_5_field() {
    // 6- and 7-field schedules both lose their leading seconds (and trailing
    // year) so the stored/crontab form is a valid 5-field expression.
    assert_eq!(normalize_schedule("0 30 9 * * 1-5"), "30 9 * * 1-5");
    assert_eq!(normalize_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
    assert_eq!(normalize_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
}

#[test]
fn validate_cron_accepts_all_documented_keywords() {
    for kw in [
        "@hourly",
        "@daily",
        "@weekly",
        "@monthly",
        "@yearly",
        "@annually",
    ] {
        assert!(validate_cron(kw).is_ok(), "{kw} should be accepted");
    }
}

#[test]
fn validate_cron_rejects_unsupported_keywords() {
    // @reboot and @midnight are documented as unsupported via the API.
    for kw in ["@reboot", "@midnight", "@nonsense"] {
        let err = validate_cron(kw);
        assert!(err.is_err(), "{kw} should be rejected");
        assert!(
            matches!(err, Err(AppError::BadRequest(_))),
            "{kw} should be rejected with BadRequest"
        );
    }
}

#[test]
fn compiled_union_uses_standard_crons_and_skips_keywords() {
    assert!(compiled_union("0 */5 * * * *").is_some());
    assert!(compiled_union("0 30 9 * * 1-5 *").is_none());
    assert!(compiled_union("@daily").is_none());
}
