
#[tokio::test]
async fn user_prompt_put_then_get_round_trips() {
    let dir = std::env::temp_dir().join(format!("moadim-user-prompt-put-{}", uuid::Uuid::new_v4()));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/user-prompt")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"always be terse"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/user-prompt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(bytes, &b"always be terse"[..]);
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn user_prompt_put_returns_500_on_write_failure() {
    // Place a regular file where the config dir should be so `create_dir_all` fails.
    let dir =
        std::env::temp_dir().join(format!("moadim-user-prompt-fail-{}", uuid::Uuid::new_v4()));
    std::fs::write(&dir, b"").unwrap();
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/user-prompt")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_file(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

#[tokio::test]
async fn user_prompt_put_returns_500_when_target_path_is_a_directory() {
    // `create_private_dir_all(parent)` succeeds here (the parent is a normal, writable
    // directory); a directory in place of the prompt file itself makes the *second* fallible
    // call, `std::fs::write(&path, ...)`, fail — the distinct `?` this test exercises versus
    // `user_prompt_put_returns_500_on_write_failure` above, which only ever reaches the first.
    let dir = std::env::temp_dir().join(format!(
        "moadim-user-prompt-targetdir-{}",
        uuid::Uuid::new_v4()
    ));
    std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    std::fs::create_dir_all(crate::paths::user_prompt_path()).unwrap();
    let app = build_app(crate::routines::new_store());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/config/user-prompt")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_dir_all(&dir);
    std::env::remove_var("MOADIM_HOME_OVERRIDE");
}

// ── max-concurrent-runs settings route tests (issue #1155) ───────────────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "max_concurrent_runs_get_defaults_when_unset_tests.rs"]
mod max_concurrent_runs_get_defaults_when_unset_tests;
