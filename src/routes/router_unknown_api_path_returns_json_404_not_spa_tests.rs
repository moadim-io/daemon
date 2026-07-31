
#[tokio::test]
async fn router_unknown_api_path_returns_json_404_not_spa() {
    // A path that matches NO route under `/api/v1` (distinct from the nonexistent-id tests,
    // which hit a real handler) must return a JSON 404 — not fall through to the SPA
    // `index.html`/200 via the outer fallback (issue #270).
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .uri("/api/v1/bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let ctype = resp.headers().get(CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("application/json"));
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "not found");
}

#[tokio::test]
async fn router_unknown_api_path_non_get_returns_404() {
    // The fallback covers every method, not just GET: a POST to an unknown `/api/v1` path
    // is a 404 too (issue #270).
    let resp = build_app(crate::routines::new_store())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
