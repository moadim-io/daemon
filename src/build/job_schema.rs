//! Generates `schemas/job.schema.json` and `schemas/job.example.toml`.

use serde_json::{json, to_string_pretty};
use std::fs;
use std::path::Path;

/// Write the job JSON Schema and an example TOML file into `<manifest_dir>/schemas/`.
pub fn generate(manifest_dir: &str) {
    let schema_dir = Path::new(manifest_dir).join("schemas");
    fs::create_dir_all(&schema_dir).expect("failed to create schemas/");

    let job_schema = json!({
        "$schema": "https://json-schema.org/draft-07/schema#",
        "title": "Job",
        "description": "Cron job configuration",
        "type": "object",
        "required": ["schedule", "handler"],
        "properties": {
            "schedule": {
                "type": "string",
                "description": "Cron expression. Supports @hourly, @daily, @weekly, @monthly, @yearly, @annually, or standard 5-field syntax (min hour dom month dow). @reboot and @midnight are not supported.",
                "examples": ["@hourly", "@daily", "30 9 * * 1-5"]
            },
            "handler": {
                "type": "string",
                "description": "Name of the handler script in ~/.config/moadim/handlers/ to run when the schedule fires. May be given with or without a file extension: resolution tries an exact match first, then appends .sh, .py, .js, .rb, .pl, .bash, .zsh (e.g. \"send-report\" matches send-report.sh).",
                "examples": ["send-report", "backup.sh"]
            },
            "metadata": {
                "type": "object",
                "description": "Arbitrary JSON key-value data stored alongside the job",
                "additionalProperties": true
            },
            "enabled": {
                "type": "boolean",
                "description": "Whether this job is active",
                "default": true
            }
        },
        "additionalProperties": false
    });

    fs::write(
        schema_dir.join("job.schema.json"),
        to_string_pretty(&job_schema).expect("failed to serialize job schema"),
    )
    .expect("failed to write schemas/job.schema.json");

    let example_toml = concat!(
        "#:schema ./job.schema.json\n",
        "\n",
        "schedule = \"30 9 * * 1-5\"\n",
        "handler  = \"my-handler\"\n",
        "enabled  = true\n",
        "\n",
        "[metadata]\n",
        "# key = \"value\"\n",
    );
    fs::write(schema_dir.join("job.example.toml"), example_toml)
        .expect("failed to write schemas/job.example.toml");
}
