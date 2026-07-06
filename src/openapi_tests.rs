#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

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

/// The `servers` URL must stay host-relative so Swagger UI "Try it out" follows the live origin
/// (a custom `MOADIM_BIND_ADDR` port or a reverse proxy) rather than a hardcoded host:port that is
/// wrong everywhere but the default bind address (issue #385).
#[test]
fn server_url_is_host_relative() {
    use utoipa::OpenApi as _;
    let servers = ApiDoc::openapi().servers.unwrap_or_default();
    let url = &servers.first().expect("a server entry is declared").url;
    assert_eq!(
        url, "/api/v1",
        "server URL must be host-relative, not absolute"
    );
}
