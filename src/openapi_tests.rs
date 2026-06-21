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

/// The `servers[0].url` must stay relative so Swagger UI "Try it out" resolves
/// against the page's own origin (works on any `MOADIM_BIND_ADDR` / behind a
/// proxy). Guards against re-introducing a hardcoded absolute address.
#[test]
fn server_url_is_relative() {
    use utoipa::OpenApi as _;
    let spec = ApiDoc::openapi();
    let servers = spec.servers.expect("spec declares a servers list");
    let url = &servers.first().expect("at least one server entry").url;
    assert!(
        url.starts_with('/'),
        "servers[0].url must be relative (origin-resolved), got {url:?}"
    );
}
