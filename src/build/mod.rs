//! Build-time code generation: JSON Schema artifacts and embedded UI HTML.

mod job_schema;
mod ui;

pub fn run(manifest_dir: &str) {
    job_schema::generate(manifest_dir);
    ui::build(manifest_dir);
}
