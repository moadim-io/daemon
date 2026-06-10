//! Build-time code generation: OpenAPI spec and JSON Schema artifacts.

/// OpenAPI spec generator.
mod job_schema;
/// Job JSON Schema generator.
mod openapi;

/// Run all code-generation steps, writing output into `manifest_dir`.
pub fn run(manifest_dir: &str) {
    openapi::generate(manifest_dir);
    job_schema::generate(manifest_dir);
}
