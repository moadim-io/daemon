//! Build script: generates `apis/openapi.json` and `schemas/job.schema.json`.

#[path = "src/build/mod.rs"]
mod build;

fn main() {
    println!("cargo:rerun-if-changed=src/routes/http.rs");
    println!("cargo:rerun-if-changed=src/cron_jobs.rs");
    println!("cargo:rerun-if-changed=src/system_cron.rs");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=schemas/job.schema.json");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    build::run(&manifest_dir);
}
