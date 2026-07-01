//! Build-time code generation: embedded UI HTML.

/// Yew UI build + inlining into a self-contained `index.html`.
mod ui;

/// Run all build-time code generation steps for `manifest_dir`.
pub fn run(manifest_dir: &str) {
    ui::build(manifest_dir);
}
