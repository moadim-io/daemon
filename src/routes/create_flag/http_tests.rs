#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt;

use crate::routes::http::build_app;

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. With no agent TOMLs present, agent validation falls back to the built-in names (so
/// `"claude"` is accepted) while `load_agent_command` finds no config — exercising the trigger
/// "no spawn" path without launching a real agent or writing into the user's real home. Tests in
/// this crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-httptest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
    }
}

#[tokio::test]
async fn router_flag_create_rejects_bad_scope() {
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schedule":"@daily","title":"Flag Scope Routine","agent":"claude","prompt":"p"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let id = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = build_app(routines.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/routines/{id}/flags"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"type":"bug","description":"d","scope":"nowhere"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
