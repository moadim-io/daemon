#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn enabled_patch_body_includes_reason_only_when_disabling() {
    let disabled = enabled_patch_body(false, Some("maintenance".to_string()));
    let disabled_json: serde_json::Value = serde_json::from_str(&disabled).unwrap();
    assert_eq!(disabled_json["enabled"], false);
    assert_eq!(disabled_json["disabled_reason"], "maintenance");

    let enabled = enabled_patch_body(true, Some("ignored".to_string()));
    let enabled_json: serde_json::Value = serde_json::from_str(&enabled).unwrap();
    assert_eq!(enabled_json["enabled"], true);
    assert!(enabled_json.get("disabled_reason").is_none());
}
