//! Shared utility functions used across multiple modules.

use std::time::SystemTime;

/// Return current Unix time in whole seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Schema override that marks a field as a free-form JSON object.
pub fn metadata_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "object", "additionalProperties": true})
}
