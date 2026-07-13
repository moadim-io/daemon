//! Build-time code generation: embedded UI HTML.

/// React `client/` build (copies its already-self-contained `vite-plugin-singlefile` output).
mod client;
mod routine_schema;

/// Yew UI build + inlining into a self-contained `index.html`.
mod ui;

/// Run all build-time code generation steps for `manifest_dir`.
pub fn run(manifest_dir: &str) {
    routine_schema::generate(manifest_dir);
    ui::build(manifest_dir);
    client::build(manifest_dir);
}
