#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use tower::ServiceExt;

use super::host_validation;
use crate::machine::tunnel_host_suffixes_tests::{temp_home, HomeGuard};

const TUNNEL_HOST: &str = "fancy-dev-run.trycloudflare.com";

/// App wired to the real `machine.local.toml` loader, as the daemon's `build_app` is.
fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(host_validation(
            vec!["localhost".to_string()],
            crate::machine::tunnel_host_suffixes,
        )))
}

/// Send one `GET /` with `host` to a clone of `app`, returning the status.
async fn get_status(app: &Router, host: &str) -> StatusCode {
    let request = Request::builder()
        .method("GET")
        .uri("/")
        .header(header::HOST, host)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

/// Overwrite `machine.local.toml` with `contents`.
fn write_config(contents: &str) {
    let config = crate::paths::machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, contents).unwrap();
}

#[tokio::test]
async fn suffix_edit_applies_to_next_request_without_restart() {
    let home = temp_home("reload-mw");
    let _home = HomeGuard::set(&home);
    let app = app();

    write_config("tunnel_host_suffixes = [\"example.dev\"]\n");
    assert_eq!(get_status(&app, TUNNEL_HOST).await, StatusCode::FORBIDDEN);
    assert_eq!(get_status(&app, "fancy.example.dev").await, StatusCode::OK);

    write_config("tunnel_host_suffixes = [\"trycloudflare.com\"]\n");
    assert_eq!(get_status(&app, TUNNEL_HOST).await, StatusCode::OK);
    assert_eq!(
        get_status(&app, "fancy.example.dev").await,
        StatusCode::FORBIDDEN
    );
}
