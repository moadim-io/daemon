use super::*;

async fn create_http_routine(routines: crate::routines::RoutineStore) -> String {
    let body = r#"{"schedule":"@daily","title":"Http Routine","agent":"claude","prompt":"p","repositories":[{"repository":"r","branch":"main"}]}"#;
    let resp = build_app(routines)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/routines")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    created["id"].as_str().unwrap().to_string()
}

async fn empty_request(
    routines: crate::routines::RoutineStore,
    method: &str,
    uri: String,
) -> axum::response::Response {
    build_app(routines)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn json_request(
    routines: crate::routines::RoutineStore,
    method: &str,
    uri: String,
    body: &'static str,
) -> axum::response::Response {
    build_app(routines)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn router_routine_lifecycle_routes() {
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let id = create_http_routine(routines.clone()).await;

    for uri in ["/api/v1/routines".to_string(), format!("/api/v1/routines/{id}")] {
        let resp = empty_request(routines.clone(), "GET", uri).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let resp = json_request(
        routines.clone(),
        "PATCH",
        format!("/api/v1/routines/{id}"),
        r#"{"title":"Patched"}"#,
    ).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = json_request(
        routines.clone(),
        "POST",
        format!("/api/v1/routines/{id}/move"),
        r#"{"folder":"http/folder","slug":"moved-routine"}"#,
    ).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = json_request(
        routines.clone(),
        "PUT",
        format!("/api/v1/routines/{id}"),
        r#"{"prompt":"replaced"}"#,
    ).await;
    assert_eq!(resp.status(), StatusCode::OK);

    for suffix in ["trigger", "scheduled-trigger"] {
        let resp = empty_request(routines.clone(), "POST", format!("/api/v1/routines/{id}/{suffix}")).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
    let resp = empty_request(routines.clone(), "GET", format!("/api/v1/routines/{id}/logs")).await;
    assert_eq!(resp.status(), StatusCode::OK);

    for uri in [
        format!("/api/v1/routines/{id}/runs"),
        "/api/v1/routines/runs".to_string(),
        "/api/v1/routines/runs?limit=5".to_string(),
    ] {
        let resp = empty_request(routines.clone(), "GET", uri).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    for suffix in ["log", "summary"] {
        let resp = empty_request(
            routines.clone(),
            "GET",
            format!("/api/v1/routines/{id}/runs/not-a-real-workbench-1/{suffix}"),
        ).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn router_routine_flags_and_delete_routes() {
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let id = create_http_routine(routines.clone()).await;

    let resp = json_request(
        routines.clone(),
        "POST",
        format!("/api/v1/routines/{id}/flags"),
        r#"{"type":"bug","description":"broken thing","scope":"general"}"#,
    ).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let flag: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let filename = flag["filename"].as_str().unwrap().to_string();

    let resp = empty_request(routines.clone(), "GET", format!("/api/v1/routines/{id}/flags")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let flags: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(flags.as_array().unwrap().len(), 1);

    let resp = empty_request(routines.clone(), "DELETE", format!("/api/v1/routines/{id}/flags/{filename}")).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = empty_request(routines.clone(), "DELETE", format!("/api/v1/routines/{id}")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!crate::paths::routine_dir(&id).exists());
}
