#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use crate::routines::new_store;
use std::sync::Mutex;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

static PATH_GUARD: Mutex<()> = Mutex::new(());

fn with_empty_path(body: impl FnOnce()) {
    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    body();
    match saved {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
}

#[test]
fn svc_create_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // With `PATH` cleared the `crontab` binary cannot be spawned, so
    // `sync_routines_to_crontab` errors and `svc_create` logs the warning but
    // still returns the created routine.
    let title = "Svc Create Sync Fail ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                goal: None,
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
    });
}

#[test]
fn svc_create_rejects_goal_over_five_lines() {
    let _home = TempHome::set();
    // A goal is meant to be a glanceable "why" (≤5 lines); a 6-line value is rejected at create
    // time with `BadRequest`, mirroring the other content bounds.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Svc Create Long Goal ZZZ".into(),
            agent: "claude".into(),
            model: None,
            prompt: "p".into(),
            goal: Some("a\nb\nc\nd\ne\nf".into()),
            repositories: vec![],
            machines: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("goal"), "got {msg:?}"),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[test]
fn svc_create_trims_and_persists_goal() {
    let _home = TempHome::set();
    // A present goal is trimmed and stored, and it survives a reload from disk.
    let title = "Svc Create Goal ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                model: None,
                prompt: "p".into(),
                goal: Some("  keep the backlog small  ".into()),
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
            },
        )
        .unwrap();
        assert_eq!(
            created.routine.goal.as_deref(),
            Some("keep the backlog small")
        );
        // Reloading the store from disk yields the same goal (persisted to routine.toml).
        let reloaded = crate::routine_storage::load_store();
        let stored = reloaded
            .lock()
            .unwrap()
            .get(&created.routine.id)
            .cloned()
            .expect("routine persisted");
        assert_eq!(stored.goal.as_deref(), Some("keep the backlog small"));
    });
}

#[test]
fn svc_update_clears_goal_with_empty_string() {
    let _home = TempHome::set();
    // `Some("")` on update clears the goal; `None` would instead keep the existing value.
    let title = "Svc Update Clear Goal ZZZ";
    let store = new_store();
    let mut routine = make_routine("upd-goal-id", title, 1, 1);
    routine.goal = Some("old goal".into());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-goal-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-goal-id",
            UpdateRoutineRequest {
                schedule: None,
                title: None,
                agent: None,
                model: None,
                prompt: None,
                goal: Some(String::new()),
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.goal, None);
    });
}

#[test]
fn svc_update_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // Same crontab-spawn failure as above, on the update path.
    let title = "Svc Update Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("upd-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-sync-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-sync-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: Some("changed".into()),
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.prompt, "changed");
    });
}

#[test]
fn svc_delete_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // Same crontab-spawn failure, on the delete path.
    let title = "Svc Delete Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("del-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("del-sync-id".into(), routine);
    with_empty_path(|| {
        let deleted = svc_delete(&store, "del-sync-id").unwrap();
        assert_eq!(deleted.routine.title, title);
    });
}

/// Build a create request with the given title and an otherwise-valid body.
fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        title: title.into(),
        agent: "claude".into(),
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

#[test]
fn svc_create_rejects_blank_and_punctuation_titles() {
    let _home = TempHome::set();
    // Covers `validate_title`'s alphanumeric-required reject branch via `svc_create`:
    // empty, whitespace-only, and punctuation-only titles all 400 before any
    // persistence or crontab sync, leaving the store empty (issue #226).
    for title in ["", "   \n\t", "!!!"] {
        let store = new_store();
        let result = svc_create(&store, create_req_with_title(title));
        assert!(
            matches!(result, Err(AppError::BadRequest(_))),
            "title {title:?} should be rejected"
        );
        assert!(store.lock().unwrap().is_empty());
    }
}

#[test]
fn svc_create_rejects_overlong_title() {
    let _home = TempHome::set();
    // Covers `validate_title`'s max-length reject branch: a title past
    // `MAX_TITLE_LEN` characters 400s even though it has alphanumerics.
    let store = new_store();
    let title = "a".repeat(MAX_TITLE_LEN + 1);
    let result = svc_create(&store, create_req_with_title(&title));
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_unknown_agent() {
    let _home = TempHome::set();
    // Covers the agent-validation branch in `svc_create`: an agent name that is
    // not in the registry must fail loud with `BadRequest` instead of being
    // persisted and silently skipped at fire time (#139).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            title: "Svc Create Unknown Agent ZZZ".into(),
            agent: "no-such-agent-zzz".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_update_rejects_blank_and_punctuation_titles() {
    let _home = TempHome::set();
    // Covers the `req.title` validation branch in `svc_update`: renaming an
    // existing routine to an empty, whitespace-only, or punctuation-only title
    // 400s and leaves the stored title untouched (issue #226).
    let original = "Svc Update Title Guard ZZZ";
    for title in ["", "   ", "!!!"] {
        let store = new_store();
        let routine = make_routine("title-guard-id", original, 1, 1);
        crate::routine_storage::write_routine(&routine).unwrap();
        store
            .lock()
            .unwrap()
            .insert("title-guard-id".into(), routine);

        let result = svc_update(
            &store,
            "title-guard-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: Some(title.into()),
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
            },
        );
        assert!(
            matches!(result, Err(AppError::BadRequest(_))),
            "update to title {title:?} should be rejected"
        );
        assert_eq!(
            store.lock().unwrap().get("title-guard-id").unwrap().title,
            original
        );
    }
}

#[test]
fn svc_create_accepts_builtin_agent() {
    let _home = TempHome::set();
    // Covers the success path of agent validation: a built-in agent
    // (`ensure_default_agents` seeds `claude`/`codex`) is accepted.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Valid Agent ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            title: title.into(),
            agent: "claude".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        },
    )
    .unwrap();
    assert_eq!(created.routine.agent, "claude");

    svc_delete(&store, &created.routine.id).unwrap();
}

#[test]
fn svc_update_rejects_unknown_agent() {
    let _home = TempHome::set();
    // Covers the agent-validation branch in `svc_update`: updating a routine's
    // agent to an unknown name must fail with `BadRequest` before persisting.
    let title = "Svc Update Unknown Agent ZZZ";
    let store = new_store();
    let routine = make_routine("upd-agent-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-agent-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-agent-id",
        UpdateRoutineRequest {
            model: None,
            schedule: None,
            title: None,
            agent: Some("no-such-agent-zzz".into()),
            prompt: None,
            goal: None,
            repositories: None,
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The stored routine keeps its original (valid) agent.
    assert_eq!(
        store.lock().unwrap().get("upd-agent-id").unwrap().agent,
        "claude"
    );
}

#[test]
fn svc_create_rejects_blank_repository_url() {
    let _home = TempHome::set();
    // Covers the repositories-validation branch in `svc_create` (#241): an entry
    // whose URL is empty or whitespace-only must fail loud with `BadRequest`
    // instead of being stored and rendered as a broken `- ` clone bullet.
    let store = new_store();
    for url in ["", "   "] {
        let result = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                schedule: "@daily".into(),
                title: "Svc Create Blank Repo ZZZ".into(),
                agent: "claude".into(),
                prompt: "p".into(),
                goal: None,
                repositories: vec![Repository {
                    repository: url.into(),
                    branch: None,
                }],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_blank_repository_branch() {
    let _home = TempHome::set();
    // Covers the optional-branch guard: a `Some` branch that is empty/whitespace
    // must be rejected so `compose_prompt` cannot emit `- url (branch )`.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            title: "Svc Create Blank Branch ZZZ".into(),
            agent: "claude".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![Repository {
                repository: "https://github.com/octocat/Hello-World".into(),
                branch: Some("  ".into()),
            }],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

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
            tags: vec![],
        },
    )
    .unwrap();
    let repo = &created.routine.repositories[0];
    assert_eq!(repo.repository, "https://github.com/octocat/Hello-World");
    assert_eq!(repo.branch.as_deref(), Some("main"));

    svc_delete(&store, &created.routine.id).unwrap();
}

#[test]
fn svc_update_rejects_blank_repository_url() {
    let _home = TempHome::set();
    // Covers the repositories-validation branch in `svc_update`: replacing the
    // list with a blank-URL entry must fail with `BadRequest` before persisting.
    let title = "Svc Update Blank Repo ZZZ";
    let store = new_store();
    let routine = make_routine("upd-repo-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-repo-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-repo-id",
        UpdateRoutineRequest {
            model: None,
            schedule: None,
            title: None,
            agent: None,
            prompt: None,
            goal: None,
            repositories: Some(vec![Repository {
                repository: " ".into(),
                branch: None,
            }]),
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The stored routine keeps its original (empty) repository list.
    assert!(store
        .lock()
        .unwrap()
        .get("upd-repo-id")
        .unwrap()
        .repositories
        .is_empty());
}
