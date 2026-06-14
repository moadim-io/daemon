//! Generates `apis/openapi.json` from a hand-authored JSON literal.

use serde_json::{json, to_string_pretty};
use std::fs;
use std::path::Path;

/// Write the OpenAPI 3.0 spec to `<manifest_dir>/apis/openapi.json`.
pub fn generate(manifest_dir: &str) {
    let out_path = Path::new(manifest_dir).join("apis/openapi.json");

    let spec = json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Moadim Server API",
            "version": "0.1.0",
            "description": "REST API for managing the user crontab"
        },
        "servers": [
            { "url": "http://127.0.0.1:5784", "description": "Local development" }
        ],
        "paths": {
            "/": {
                "get": {
                    "summary": "Liveness check",
                    "operationId": "index",
                    "responses": {
                        "200": {
                            "description": "Server is running",
                            "content": {
                                "text/plain": {
                                    "schema": { "type": "string", "example": "Moadim server is running" }
                                }
                            }
                        }
                    }
                }
            },
            "/health": {
                "get": {
                    "summary": "Health check",
                    "operationId": "health",
                    "responses": {
                        "200": {
                            "description": "Health status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/echo": {
                "post": {
                    "summary": "Echo a message back with server timestamp",
                    "operationId": "echo",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/EchoRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Echoed message with timestamp",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/EchoResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" }
                    }
                }
            },
            "/cron-jobs": {
                "get": {
                    "summary": "List all cron jobs",
                    "description": "Returns all entries from the user crontab. Managed entries (created by moadim) have source=\"managed\"; pre-existing entries have source=\"system\".",
                    "operationId": "listCronJobs",
                    "responses": {
                        "200": {
                            "description": "Array of cron jobs",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/CronJob" }
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Add a managed cron job",
                    "description": "Appends a new entry to the user crontab tagged with a moadim ID comment.",
                    "operationId": "createCronJob",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateCronJobRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Created cron job",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CronJob" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" }
                    }
                }
            },
            "/cron-jobs/{id}": {
                "parameters": [
                    {
                        "name": "id",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }
                ],
                "get": {
                    "summary": "Get a cron job by ID",
                    "operationId": "getCronJob",
                    "responses": {
                        "200": {
                            "description": "Cron job",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CronJob" }
                                }
                            }
                        },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                },
                "put": {
                    "summary": "Update a managed cron job (full)",
                    "operationId": "updateCronJobPut",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UpdateCronJobRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated cron job",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CronJob" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                },
                "patch": {
                    "summary": "Update a managed cron job (partial)",
                    "operationId": "updateCronJobPatch",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UpdateCronJobRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated cron job",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CronJob" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                },
                "delete": {
                    "summary": "Remove a managed cron job",
                    "description": "Removes the moadim tag comment and cron entry from the user crontab. Only managed entries can be deleted.",
                    "operationId": "deleteCronJob",
                    "responses": {
                        "200": {
                            "description": "Deleted cron job",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CronJob" }
                                }
                            }
                        },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "HealthResponse": {
                    "type": "object",
                    "required": ["status", "uptime_secs", "running"],
                    "properties": {
                        "status": { "type": "string", "example": "ok" },
                        "uptime_secs": { "type": "integer", "format": "int64", "minimum": 0 },
                        "running": { "type": "boolean" }
                    }
                },
                "EchoRequest": {
                    "type": "object",
                    "required": ["message"],
                    "properties": {
                        "message": { "type": "string" }
                    }
                },
                "EchoResponse": {
                    "type": "object",
                    "required": ["message", "timestamp"],
                    "properties": {
                        "message": { "type": "string" },
                        "timestamp": { "type": "integer", "format": "int64", "minimum": 0 }
                    }
                },
                "CronJob": {
                    "type": "object",
                    "required": ["id", "schedule", "command", "source"],
                    "properties": {
                        "id": { "type": "string" },
                        "schedule": { "type": "string", "example": "@daily" },
                        "command": { "type": "string", "example": "/usr/bin/backup.sh" },
                        "source": {
                            "type": "string",
                            "enum": ["managed", "system"],
                            "description": "\"managed\" for moadim-owned entries; \"system\" for pre-existing entries"
                        }
                    }
                },
                "CreateCronJobRequest": {
                    "type": "object",
                    "required": ["schedule", "command"],
                    "properties": {
                        "schedule": { "type": "string", "example": "30 9 * * 1-5" },
                        "command": { "type": "string", "example": "/usr/bin/backup.sh" }
                    }
                },
                "UpdateCronJobRequest": {
                    "type": "object",
                    "properties": {
                        "schedule": { "type": "string", "nullable": true },
                        "command": { "type": "string", "nullable": true }
                    }
                },
                "ErrorResponse": {
                    "type": "object",
                    "required": ["error"],
                    "properties": {
                        "error": { "type": "string" }
                    }
                }
            },
            "responses": {
                "BadRequest": {
                    "description": "Bad request",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "NotFound": {
                    "description": "Resource not found",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                }
            }
        }
    });

    let json = to_string_pretty(&spec).expect("failed to serialize OpenAPI spec");
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).expect("failed to create apis/");
    }
    fs::write(&out_path, json).expect("failed to write apis/openapi.json");
}
