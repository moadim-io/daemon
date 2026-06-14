//! Build script: generates `schemas/job.schema.json` and the embedded UI HTML.
//! The OpenAPI spec (`apis/openapi.json`) is generated at runtime from utoipa decorators.

#[path = "src/build/mod.rs"]
mod build;

fn main() {
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=schemas/job.schema.json");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    build::run(&manifest_dir);
}
