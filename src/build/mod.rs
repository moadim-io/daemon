mod job_schema;
mod openapi;

pub fn run(manifest_dir: &str) {
    openapi::generate(manifest_dir);
    job_schema::generate(manifest_dir);
}
