//! Build-time code generation: job JSON Schema, example TOML, and embedded UI HTML.
//! The OpenAPI spec (`apis/openapi.json`) is generated at runtime from utoipa decorators.

/// Job JSON Schema generator.
mod job_schema;
/// UI HTML builder.
mod ui;

/// Run all code-generation steps, writing output into `manifest_dir`.
pub fn run(manifest_dir: &str) {
    job_schema::generate(manifest_dir);
    ui::build(manifest_dir);
}
