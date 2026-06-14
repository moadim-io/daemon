//! Build-time code generation: JSON Schema artifacts and embedded UI HTML.

mod job_schema;
/// Yew UI build + inlining into a self-contained `index.html`.
mod ui;

/// Run all build-time code generation steps for `manifest_dir`.
pub fn run(manifest_dir: &str) {
    job_schema::generate(manifest_dir);
    ui::build(manifest_dir);
}
