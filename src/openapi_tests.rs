#![allow(clippy::missing_docs_in_private_items)]

use super::ApiDoc;

/// Keep the committed `apis/openapi.json` in sync with the path decorators. On drift this
/// rewrites the file (so `cargo test` regenerates it) and then fails so the change is committed.
#[test]
fn committed_spec_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/apis/openapi.json");
    let generated = ApiDoc::to_json();
    let committed = std::fs::read_to_string(path).unwrap_or_default();
    if committed != generated {
        std::fs::write(path, &generated).unwrap();
        panic!("apis/openapi.json was stale — regenerated; re-run tests and commit the change");
    }
}
