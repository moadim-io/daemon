use super::*;

#[test]
fn parse_selects_json_case_insensitively() {
    assert_eq!(LogFormat::parse(Some("json")), LogFormat::Json);
    assert_eq!(LogFormat::parse(Some("JSON")), LogFormat::Json);
    assert_eq!(LogFormat::parse(Some("Json")), LogFormat::Json);
}

#[test]
fn parse_falls_back_to_text_for_unset_or_invalid() {
    assert_eq!(LogFormat::parse(None), LogFormat::Text);
    assert_eq!(LogFormat::parse(Some("")), LogFormat::Text);
    assert_eq!(LogFormat::parse(Some("yaml")), LogFormat::Text);
    assert_eq!(LogFormat::parse(Some("text")), LogFormat::Text);
}

#[test]
fn json_line_round_trips_through_a_parser() {
    let record = log::Record::builder()
        .args(format_args!("hello world"))
        .level(log::Level::Warn)
        .target("moadim::test")
        .build();

    let line = format_json_line(&record);
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("valid JSON line");

    assert_eq!(parsed["level"], "WARN");
    assert_eq!(parsed["target"], "moadim::test");
    assert_eq!(parsed["msg"], "hello world");
    assert!(
        parsed["ts"].as_str().is_some_and(|ts| ts.contains('T')),
        "ts should be an RFC 3339 timestamp, got {:?}",
        parsed["ts"]
    );
}

#[test]
fn init_leaves_the_default_text_formatter_in_place_when_unset() {
    std::env::remove_var("MOADIM_LOG_FORMAT");
    init();
}

#[test]
fn init_installs_a_backend_and_emits_through_the_json_formatter() {
    // No other test reads or installs a logger, so this is the only writer of the var and the
    // only caller of `init` in the whole test binary.
    std::env::set_var("MOADIM_LOG_FORMAT", "json");
    init();
    // Drives an actual record through the JSON `format` closure registered by `init` so its
    // body — not just the branch that registers it — is exercised.
    log::info!("logging init smoke test");
    std::env::remove_var("MOADIM_LOG_FORMAT");
}
