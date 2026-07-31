#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_create_trims_repository_entries() {
    let _home = TempHome::set();
    // Covers the normalization path: surrounding whitespace on a valid URL/branch
    // is trimmed before storing, so the rendered preamble bullet is clean.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Trim Repo ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            schedules: vec![],
            title: title.into(),
            agent: "claude".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![Repository {
                repository: "  https://github.com/octocat/Hello-World  ".into(),
                branch: Some("  main  ".into()),
            }],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            power_saving_exempt: false,
            tags: vec![],
            env: std::collections::HashMap::new(),
            failure_threshold: None,
        },
    )
    .unwrap();
    let repo = &created.routine.repositories[0];
    assert_eq!(repo.repository, "https://github.com/octocat/Hello-World");
    assert_eq!(repo.branch.as_deref(), Some("main"));

    svc_delete(&store, &created.routine.id).unwrap();
}

#[test]
fn svc_create_rejects_invalid_env_key() {
    let _home = TempHome::set();
    // Covers `validate_env`'s key-shape reject branch via `svc_create` (#408): a key that isn't a
    // POSIX shell identifier must 400 before anything is persisted or the crontab is touched.
    let store = new_store();
    let mut req = create_req_with_title("Svc Create Invalid Env Key");
    req.env = std::collections::HashMap::from([("not-valid".to_string(), "x".to_string())]);
    let result = svc_create(&store, req);
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_env_value_with_newline() {
    let _home = TempHome::set();
    // Covers `validate_env`'s newline-injection reject branch (#408): a value carrying a newline
    // could otherwise inject an extra statement into the single-line, `;`-joined launch command.
    let store = new_store();
    let mut req = create_req_with_title("Svc Create Env Newline Value");
    req.env = std::collections::HashMap::from([("KEY".to_string(), "a\nb".to_string())]);
    let result = svc_create(&store, req);
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_persists_valid_env_and_redacts_it_from_the_json_response() {
    let _home = TempHome::set();
    // A valid `[env]` map is accepted, persisted on the in-memory `Routine`, and reachable via
    // `RoutineResponse::env_keys` (names only) — but the values must never survive a JSON
    // serialization of the response (#408: "secret values never appear in API responses").
    let store = new_store();
    let mut req = create_req_with_title("Svc Create Env Redaction");
    req.env =
        std::collections::HashMap::from([("MY_TOKEN".to_string(), "super-secret".to_string())]);
    let created = svc_create(&store, req).unwrap();

    assert_eq!(
        created.routine.env.get("MY_TOKEN").map(String::as_str),
        Some("super-secret"),
        "the resolved value must still be available in-process for command building"
    );
    assert_eq!(created.env_keys, vec!["MY_TOKEN".to_string()]);

    let json = serde_json::to_value(&created).unwrap();
    let rendered = json.to_string();
    assert!(
        !rendered.contains("super-secret"),
        "the secret value must never appear in the serialized API response: {rendered}"
    );
    assert!(
        json.get("env").is_none(),
        "the raw env map must be entirely absent from the serialized response, got: {rendered}"
    );
    assert_eq!(
        json["env_keys"],
        serde_json::json!(["MY_TOKEN"]),
        "the key name alone must still be surfaced: {rendered}"
    );

    svc_delete(&store, &created.routine.id).unwrap();
}
