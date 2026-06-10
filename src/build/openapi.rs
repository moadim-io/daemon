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
            "description": "REST API for managing cron jobs"
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
                    "operationId": "listCronJobs",
                    "responses": {
                        "200": {
                            "description": "Array of cron jobs sorted by creation time",
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
                    "summary": "Create a cron job",
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
            "/system-cron-jobs": {
                "get": {
                    "summary": "List read-only system cron jobs",
                    "description": "Returns cron jobs discovered from the host system (crontab -l, /etc/crontab, /etc/cron.d/*). Not managed by this server — cannot be created, updated, or deleted.",
                    "operationId": "listSystemCronJobs",
                    "responses": {
                        "200": {
                            "description": "Array of read-only system cron jobs",
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
                }
            },
            "/cron-jobs/{id}": {
                "parameters": [
                    {
                        "name": "id",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string", "format": "uuid" }
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
                    "summary": "Update a cron job (full)",
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
                    "summary": "Update a cron job (partial)",
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
                    "summary": "Delete a cron job",
                    "operationId": "deleteCronJob",
                    "responses": {
                        "204": { "description": "Deleted" },
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
                    "required": ["id", "schedule", "handler", "metadata", "enabled", "source", "created_at", "updated_at"],
                    "properties": {
                        "id": { "type": "string" },
                        "schedule": { "type": "string", "example": "@hourly" },
                        "handler": { "type": "string" },
                        "metadata": { },
                        "enabled": { "type": "boolean" },
                        "source": {
                            "type": "string",
                            "description": "\"managed\" for server-owned jobs; \"system:*\" for read-only system cron entries",
                            "example": "managed"
                        },
                        "created_at": { "type": "integer", "format": "int64", "minimum": 0 },
                        "updated_at": { "type": "integer", "format": "int64", "minimum": 0 }
                    }
                },
                "CreateCronJobRequest": {
                    "type": "object",
                    "required": ["schedule", "handler"],
                    "properties": {
                        "schedule": { "type": "string", "example": "0 30 9 * * 1-5 *" },
                        "handler": { "type": "string" },
                        "metadata": { },
                        "enabled": { "type": "boolean", "default": true }
                    }
                },
                "UpdateCronJobRequest": {
                    "type": "object",
                    "properties": {
                        "schedule": { "type": "string", "nullable": true },
                        "handler": { "type": "string", "nullable": true },
                        "metadata": { "nullable": true },
                        "enabled": { "type": "boolean", "nullable": true }
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
    fs::write(&out_path, json).expect("failed to write apis/openapi.json");
}
