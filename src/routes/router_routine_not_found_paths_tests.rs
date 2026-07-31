#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[tokio::test]
async fn router_routine_not_found_paths() {
    for (method, suffix) in [
        ("GET", ""),
        ("DELETE", ""),
        ("POST", "/trigger"),
        ("POST", "/scheduled-trigger"),
        ("GET", "/prompt-preview"),
        ("GET", "/logs"),
        ("GET", "/runs"),
        ("GET", "/runs/some-workbench-1/log"),
        ("GET", "/runs/some-workbench-1/summary"),
    ] {
        let resp = build_app(crate::routines::new_store())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/api/v1/routines/no-such{suffix}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method} {suffix}");
    }

    // PATCH nonexistent
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/routines/no-such")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"title":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
