#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn routine_body_serializes_all_fields() {
    let value: Value = serde_json::from_str(
        &routine_body(
            vec!["* * * * *".to_string()],
            "title".into(),
            "agent".into(),
            Some("claude-sonnet-4-6".into()),
            "prompt".into(),
            Some("keep it small".into()),
            Some("[]".into()),
            Some("[\"work\"]".into()),
            Some(30),
            Some(60),
            vec!["triage".to_string(), "nightly".to_string()],
            false,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(value["title"], Value::String("title".to_string()));
    assert_eq!(value["goal"], Value::String("keep it small".to_string()));
    assert_eq!(
        value["model"],
        Value::String("claude-sonnet-4-6".to_string())
    );
    assert_eq!(value["repositories"], Value::Array(vec![]));
    assert_eq!(
        value["machines"],
        Value::Array(vec![Value::String("work".to_string())])
    );
    assert_eq!(value["ttl_secs"], Value::from(30));
    assert_eq!(
        value["tags"],
        Value::Array(vec![
            Value::String("triage".to_string()),
            Value::String("nightly".to_string()),
        ])
    );
    assert_eq!(value["enabled"], Value::Bool(true));
}

#[test]
fn routine_body_serializes_disabled_reason_only_when_disabled() {
    let disabled: Value = serde_json::from_str(
        &routine_body(
            vec!["* * * * *".to_string()],
            "title".into(),
            "agent".into(),
            None,
            "prompt".into(),
            None,
            None,
            None,
            None,
            None,
            vec![],
            true,
            Some("maintenance".to_string()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(disabled["enabled"], Value::Bool(false));
    assert_eq!(disabled["disabled_reason"], Value::String("maintenance".to_string()));

    let enabled: Value = serde_json::from_str(
        &routine_body(
            vec!["* * * * *".to_string()],
            "title".into(),
            "agent".into(),
            None,
            "prompt".into(),
            None,
            None,
            None,
            None,
            None,
            vec![],
            false,
            Some("ignored".to_string()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(enabled["enabled"], Value::Bool(true));
    assert!(enabled.get("disabled_reason").is_none());
}

#[test]
fn routine_body_rejects_bad_repositories() {
    assert_eq!(
        routine_body(
            vec!["* * * * *".to_string()],
            "t".into(),
            "a".into(),
            None,
            "p".into(),
            None,
            Some("{bad".into()),
            None,
            None,
            None,
            vec![],
            false,
            None,
        ),
        Err(2)
    );
}

#[test]
fn routine_body_rejects_bad_machines() {
    // Covers the `?` error branch on the `machines` insert_json_opt call (L509).
    assert_eq!(
        routine_body(
            vec!["* * * * *".to_string()],
            "t".into(),
            "a".into(),
            None,
            "p".into(),
            None,
            None,
            Some("{bad".into()),
            None,
            None,
            vec![],
            false,
            None,
        ),
        Err(2)
    );
}
