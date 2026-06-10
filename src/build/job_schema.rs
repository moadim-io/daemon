use serde_json::{json, to_string_pretty};
use std::fs;
use std::path::Path;

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
                "description": "Cron expression. Supports @hourly, @daily, @weekly, @monthly, or 7-field syntax (sec min hour dom month dow year).",
                "examples": ["@hourly", "@daily", "0 30 9 * * 1-5 *"]
            },
            "handler": {
                "type": "string",
                "description": "Handler identifier invoked when the schedule fires"
            },
            "metadata": {
                "type": "object",
                "description": "Arbitrary key-value data passed to the handler",
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
        "schedule = \"0 30 9 * * 1-5 *\"\n",
        "handler  = \"my-handler\"\n",
        "enabled  = true\n",
        "\n",
        "[metadata]\n",
        "# key = \"value\"\n",
    );
    fs::write(schema_dir.join("job.example.toml"), example_toml)
        .expect("failed to write schemas/job.example.toml");
}
