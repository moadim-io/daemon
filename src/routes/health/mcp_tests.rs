#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::MoadimMcp;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        std::sync::Arc::new(tokio::sync::Notify::new()),
    )
}

#[test]
fn health_returns_success() {
    let handler = make_handler();
    let result = handler.health().unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn health_content_contains_status() {
    let handler = make_handler();
    let result = handler.health().unwrap();
    let text = &result.content[0];
    let json_str = match &text {
        rmcp::model::ContentBlock::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["status"], "ok");
    assert_eq!(val["running"], true);
    // Build provenance is surfaced for parity with `GET /health` and `--version`.
    assert_eq!(val["version"], crate::build_info::VERSION);
    assert_eq!(val["git_sha"], crate::build_info::GIT_SHA);
    assert_eq!(val["build_date"], crate::build_info::BUILD_DATE);
    // Resolved machine identity, for parity with `GET /health`.
    assert!(
        val["machine"].is_string() && !val["machine"].as_str().unwrap().is_empty(),
        "mcp health should carry a non-empty machine name, got: {val}"
    );
}
