#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[test]
fn produces_object_type_schema() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = metadata_schema(&mut gen);
    let val = serde_json::to_value(schema).unwrap();
    assert_eq!(val["type"], "object");
}

#[test]
fn allows_additional_properties() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = metadata_schema(&mut gen);
    let val = serde_json::to_value(schema).unwrap();
    assert_eq!(val["additionalProperties"], true);
}
