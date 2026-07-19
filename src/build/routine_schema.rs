//! Generates `schemas/routine.schema.json` and `schemas/routine.example.toml`.
//!
//! A `routine.toml` is normally written by the daemon (one folder per routine under the config
//! tree), but a committed JSON Schema gives editors validation/completion and documents the
//! on-disk shape. The shape tracks the `RoutineToml` serializer in `src/routine_storage.rs`.

use serde_json::{json, to_string_pretty};
use std::fs;
use std::path::Path;

/// Write the routine JSON Schema and an example TOML file into `<manifest_dir>/schemas/`.
pub fn generate(manifest_dir: &str) {
    let schema_dir = Path::new(manifest_dir).join("schemas");
    fs::create_dir_all(&schema_dir).expect("failed to create schemas/");

    let routine_schema = json!({
        "$schema": "https://json-schema.org/draft-07/schema#",
        "title": "Routine",
        "description": "A scheduled AI-agent task. Normally written and maintained by the daemon (one routine.toml per routine); this schema documents its on-disk shape and powers editor validation. Runtime trigger state lives in a gitignored state.local.toml sidecar, not here.",
        "type": "object",
        "required": ["schedule", "title", "agent", "prompt"],
        "properties": {
            "id": {
                "type": "string",
                "description": "UUID v4 uniquely identifying the routine, stable across renames. Assigned by the daemon on create."
            },
            "schedule": {
                "type": "string",
                "description": "Cron expression for when the routine runs, evaluated in the host's local system timezone (the OS crontab timezone), not UTC.",
                "examples": ["@hourly", "@daily", "30 9 * * 1-5"]
            },
            "title": {
                "type": "string",
                "description": "Human name; slugified to name the workbench folder and tmux session."
            },
            "agent": {
                "type": "string",
                "description": "Agent registry key (e.g. \"claude\") resolved from the agents config directory."
            },
            "prompt": {
                "type": "string",
                "description": "Task prompt handed to the agent."
            },
            "repositories": {
                "type": "array",
                "description": "Git repositories listed to the agent as prompt context (not cloned by moadim).",
                "items": {
                    "type": "object",
                    "required": ["repository"],
                    "properties": {
                        "repository": {
                            "type": "string",
                            "description": "Git remote URL."
                        },
                        "branch": {
                            "type": "string",
                            "description": "Branch to use; omit for the remote default branch."
                        }
                    },
                    "additionalProperties": false
                }
            },
            "enabled": {
                "type": "boolean",
                "description": "Whether the routine is active.",
                "default": true
            },
            "created_at": {
                "type": "integer",
                "minimum": 0,
                "description": "Unix timestamp (seconds) when the routine was created. Daemon-assigned."
            },
            "updated_at": {
                "type": "integer",
                "minimum": 0,
                "description": "Unix timestamp (seconds) when the routine was last updated. Daemon-assigned."
            },
            "ttl_secs": {
                "type": "integer",
                "minimum": 0,
                "description": "Workbench retention (seconds) for finished runs; caps the cron-derived retention lower. Absent uses the daemon default."
            },
            "max_runtime_secs": {
                "type": "integer",
                "minimum": 0,
                "description": "Max wall-clock seconds a single run may execute before the watchdog kills its hung session. Absent uses the daemon default."
            },
            "last_triggered_at": {
                "type": "integer",
                "minimum": 0,
                "description": "Legacy/read-only. Pre-rename last-manual-trigger timestamp; migrated into the state.local.toml sidecar on the next write and never written back. Accepted only so routine.toml files from older daemons still load."
            },
            "last_manual_trigger_at": {
                "type": "integer",
                "minimum": 0,
                "description": "Legacy/read-only. Last-manual-trigger timestamp; runtime trigger state now lives in the gitignored state.local.toml sidecar and is never written to routine.toml."
            }
        },
        "additionalProperties": false
    });

    fs::write(
        schema_dir.join("routine.schema.json"),
        to_string_pretty(&routine_schema).expect("failed to serialize routine schema"),
    )
    .expect("failed to write schemas/routine.schema.json");

    let example_toml = concat!(
        "#:schema ./routine.schema.json\n",
        "\n",
        "schedule = \"30 9 * * 1-5\"\n",
        "title    = \"My routine\"\n",
        "agent    = \"claude\"\n",
        "prompt   = \"Describe the task for the agent here.\"\n",
        "enabled  = true\n",
        "\n",
        "# Repositories listed to the agent as prompt context (optional).\n",
        "# [[repositories]]\n",
        "# repository = \"https://github.com/owner/repo\"\n",
        "# branch     = \"main\"\n",
    );
    fs::write(schema_dir.join("routine.example.toml"), example_toml)
        .expect("failed to write schemas/routine.example.toml");
}
