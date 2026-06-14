//! Generates `schemas/job.schema.json` and `schemas/job.example.toml`.

use serde_json::{json, to_string_pretty};
use std::fs;
use std::path::Path;

/// Write the cron job JSON Schema into `<manifest_dir>/schemas/`.
pub fn generate(manifest_dir: &str) {
    let schema_dir = Path::new(manifest_dir).join("schemas");
    fs::create_dir_all(&schema_dir).expect("failed to create schemas/");

    let job_schema = json!({
        "$schema": "https://json-schema.org/draft-07/schema#",
        "title": "CronJob",
        "description": "A cron entry in the user crontab managed by moadim",
        "type": "object",
        "required": ["id", "schedule", "command", "source"],
        "properties": {
            "id": {
                "type": "string",
                "description": "Stable identifier (UUID for managed entries, deterministic hash for system entries)"
            },
            "schedule": {
                "type": "string",
                "description": "5-field cron expression or @keyword",
                "examples": ["@hourly", "@daily", "30 9 * * 1-5"]
            },
            "command": {
                "type": "string",
                "description": "The command the OS executes"
            },
            "source": {
                "type": "string",
                "description": "\"managed\" for entries owned by moadim; \"system\" for pre-existing entries",
                "enum": ["managed", "system"]
            }
        },
        "additionalProperties": false
    });

    fs::write(
        schema_dir.join("job.schema.json"),
        to_string_pretty(&job_schema).expect("failed to serialize job schema"),
    )
    .expect("failed to write schemas/job.schema.json");
}
