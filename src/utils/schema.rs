/// Schema override that marks a field as a free-form JSON object.
pub fn metadata_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "object", "additionalProperties": true})
}
