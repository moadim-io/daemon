#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::available_agents;
use croner::Cron;

#[test]
fn ships_at_least_one_default() {
    assert!(!DEFAULT_ROUTINES.is_empty());
}

#[test]
fn first_default_updates_moadim_cargo_package() {
    let first = &DEFAULT_ROUTINES[0];
    assert_eq!(first.title, "Update moadim cargo package");
    assert!(first.prompt.contains("cargo install moadim --force"));
}

#[test]
fn second_default_is_the_1_percent() {
    let spec = &DEFAULT_ROUTINES[1];
    assert_eq!(spec.title, "The 1 Percent");
    assert!(spec.prompt.contains("list_routines"));
    assert!(spec.prompt.contains("update_routine"));
    assert!(spec.prompt.contains("NOT_REPO"));
}

#[test]
fn third_default_is_token_trim() {
    let spec = &DEFAULT_ROUTINES[2];
    assert_eq!(spec.title, "Token Trim");
    assert!(spec.prompt.contains("list_routines"));
    assert!(spec.prompt.contains("update_routine"));
    assert!(spec.prompt.contains("NOT_REPO"));
    assert!(spec.prompt.contains("token"));
}

#[test]
fn every_schedule_is_a_valid_cron() {
    for spec in DEFAULT_ROUTINES {
        let normalized = normalize_schedule(spec.schedule);
        assert!(
            normalized.parse::<Cron>().is_ok(),
            "schedule for {:?} is not a valid cron: {normalized:?}",
            spec.title
        );
    }
}

#[test]
fn every_agent_is_a_known_builtin() {
    let known = available_agents();
    for spec in DEFAULT_ROUTINES {
        assert!(
            known.iter().any(|agent| agent == spec.agent),
            "agent {:?} for routine {:?} is not a built-in agent",
            spec.agent,
            spec.title
        );
    }
}

#[test]
fn materialize_stamps_timestamps_and_marks_managed() {
    let spec = &DEFAULT_ROUTINES[0];
    let routine = materialize(spec, 1234);
    assert_eq!(routine.created_at, 1234);
    assert_eq!(routine.updated_at, 1234);
    assert_eq!(routine.source, "managed");
    assert!(routine.enabled);
    assert!(routine.last_manual_trigger_at.is_none());
    assert!(!routine.id.is_empty());
    // Schedule is normalized, not the raw spec string.
    assert_eq!(routine.schedule, normalize_schedule(spec.schedule));
}

#[test]
fn materialize_assigns_unique_ids() {
    let spec = &DEFAULT_ROUTINES[0];
    assert_ne!(materialize(spec, 0).id, materialize(spec, 0).id);
}

#[test]
fn reconcile_returns_none_when_up_to_date() {
    let spec = &DEFAULT_ROUTINES[0];
    let cur = materialize(spec, 100);
    assert!(reconcile(spec, &cur, 200).is_none());
}

#[test]
fn reconcile_preserves_disabled_toggle() {
    let spec = &DEFAULT_ROUTINES[0];
    // User turned the default off and an old prompt is on disk: it must be refreshed but stay off.
    let mut cur = materialize(spec, 100);
    cur.enabled = false;
    cur.prompt = "stale prompt".to_string();
    let updated = reconcile(spec, &cur, 200).expect("drifted routine should be rewritten");
    assert!(
        !updated.enabled,
        "must not re-enable a user-disabled default"
    );
    assert_eq!(updated.prompt, spec.prompt, "prompt should be refreshed");
}

#[test]
fn reconcile_preserves_power_saving() {
    let spec = &DEFAULT_ROUTINES[0];
    // Power saving is daemon/policy-owned, not spec-derived — a content drift refresh must not
    // clear it, the same way it must not touch `enabled`.
    let mut cur = materialize(spec, 100);
    cur.power_saving = true;
    cur.prompt = "stale prompt".to_string();
    let updated = reconcile(spec, &cur, 200).expect("drifted routine should be rewritten");
    assert!(
        updated.power_saving,
        "must not clear power-saving state on a content refresh"
    );
}

#[test]
fn reconcile_refreshes_content_but_keeps_identity() {
    let spec = &DEFAULT_ROUTINES[0];
    let mut cur = materialize(spec, 100);
    cur.schedule = "0 0 * * *".to_string();
    let updated = reconcile(spec, &cur, 200).expect("schedule drift should be rewritten");
    assert_eq!(updated.schedule, normalize_schedule(spec.schedule));
    // Identity and history are carried over; only updated_at advances.
    assert_eq!(updated.id, cur.id);
    assert_eq!(updated.created_at, cur.created_at);
    assert_eq!(updated.updated_at, 200);
}

#[test]
fn reconcile_keeps_enabled_default_enabled() {
    let spec = &DEFAULT_ROUTINES[0];
    let mut cur = materialize(spec, 100);
    cur.prompt = "stale".to_string();
    let updated = reconcile(spec, &cur, 200).expect("drift should be rewritten");
    assert!(updated.enabled);
}

#[test]
fn reconcile_treats_empty_machines_as_drift_and_seeds_current_machine() {
    // Legacy default routines seeded before machine-awareness were stored with an empty
    // `machines` list, leaving them permanently dormant. `reconcile` must detect this
    // as drift (even when all other daemon-owned fields are current) and seed the current
    // machine so the routine becomes active. (#723)
    let spec = &DEFAULT_ROUTINES[0];
    let mut cur = materialize(spec, 100);
    cur.machines = Vec::new(); // simulate pre-machine-awareness legacy state
    let updated = reconcile(spec, &cur, 200)
        .expect("empty machines list must be treated as drift and trigger a rewrite");
    assert!(
        !updated.machines.is_empty(),
        "reconcile must seed the current machine when cur.machines is empty"
    );
}

#[test]
fn reconcile_returns_none_when_machines_already_set_and_otherwise_current() {
    // A correctly seeded routine (non-empty machines, current content) must NOT be rewritten
    // just because reconcile now inspects the machines list.
    let spec = &DEFAULT_ROUTINES[0];
    let cur = materialize(spec, 100);
    assert!(
        !cur.machines.is_empty(),
        "materialize must assign a machine — test pre-condition"
    );
    assert!(
        reconcile(spec, &cur, 200).is_none(),
        "a routine with current content and a non-empty machines list must not trigger a rewrite"
    );
}

#[test]
fn materialize_assigns_non_empty_machines_list() {
    // materialize must always seed the current machine so a freshly created default runs
    // immediately instead of being dormant (#723).
    let spec = &DEFAULT_ROUTINES[0];
    let routine = materialize(spec, 0);
    assert!(
        !routine.machines.is_empty(),
        "materialize must assign the current machine to a freshly seeded default routine"
    );
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A unique, not-yet-created scratch home directory under the system temp dir.
fn scratch_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-defaults-{}", uuid::Uuid::new_v4()))
}

/// Run `body` with `HOME` redirected at a fresh temp home (so `crate::paths` resolves all
/// config/routines paths under it), restoring the previous value and removing the temp home
/// afterwards. The crate's tests run single-threaded, so mutating the process-global `HOME` here is
/// safe. `dirs::home_dir()` — which every `crate::paths` builder consults — reads `$HOME` on this
/// platform, so redirecting it points `routines_dir()` (and thus `write_routine`) at the tempdir.
fn with_redirected_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_home();
    std::fs::create_dir_all(&home).unwrap();
    let previous_home = std::env::var_os("HOME");
    let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: tests in this crate run single-threaded per binary; we set and immediately restore the
    // overrides around this call. XDG_CONFIG_HOME is also redirected so config_root() uses the
    // temp home rather than a CI runner's real XDG path.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
    }
    body(&home);
    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match previous_xdg {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

/// An empty in-memory routine store.
fn empty_store() -> RoutineStore {
    Arc::new(Mutex::new(HashMap::new()))
}

#[test]
fn ensure_default_routines_seeds_empty_store() {
    // (a) Empty store → materialize + write + insert: the routine lands on disk and in the store.
    with_redirected_home(|_home| {
        let store = empty_store();
        ensure_default_routines(&store);

        let seeded = store.lock().unwrap();
        let spec = &DEFAULT_ROUTINES[0];
        let slug = slugify(spec.title);
        let routine = seeded
            .values()
            .find(|routine| slugify(&routine.title) == slug)
            .expect("default routine must be seeded into the store");
        assert_eq!(routine.title, spec.title);
        assert_eq!(routine.source, "managed");
        assert!(routine.enabled);
        // The routine's directory was written under the redirected home.
        assert!(crate::paths::routine_dir(&slug).is_dir());
    });
}

#[test]
fn ensure_default_routines_skips_up_to_date_existing() {
    // (b) Existing up-to-date routine → reconcile returns None → `continue`: the store is left
    // untouched (same id, no extra entries).
    with_redirected_home(|_home| {
        let spec = &DEFAULT_ROUTINES[0];
        let existing = materialize(spec, now_secs());
        let existing_id = existing.id.clone();
        let store = empty_store();
        store.lock().unwrap().insert(existing.id.clone(), existing);

        ensure_default_routines(&store);

        let after = store.lock().unwrap();
        // The existing up-to-date routine must not be duplicated (still exactly one entry with that
        // slug). Other defaults may have been seeded alongside it.
        let slug = slugify(spec.title);
        let slug_count = after
            .values()
            .filter(|routine| slugify(&routine.title) == slug)
            .count();
        assert_eq!(slug_count, 1, "up-to-date default must not be duplicated");
        assert!(
            after.contains_key(&existing_id),
            "the original entry must be preserved unchanged"
        );
    });
}

#[test]
fn ensure_default_routines_rewrites_drifted_existing() {
    // (c) Existing drifted routine → reconcile returns Some → rewrite path: identity is preserved
    // but the daemon-owned content is refreshed to the spec.
    with_redirected_home(|_home| {
        let spec = &DEFAULT_ROUTINES[0];
        let mut existing = materialize(spec, now_secs());
        let existing_id = existing.id.clone();
        existing.prompt = "stale prompt".to_string();
        existing.schedule = "0 0 * * *".to_string();
        let store = empty_store();
        store.lock().unwrap().insert(existing.id.clone(), existing);

        ensure_default_routines(&store);

        let after = store.lock().unwrap();
        // The drifted routine must be updated in-place, not duplicated (still exactly one entry
        // with that slug). Other defaults may have been seeded alongside it.
        let slug = slugify(spec.title);
        let slug_count = after
            .values()
            .filter(|routine| slugify(&routine.title) == slug)
            .count();
        assert_eq!(slug_count, 1, "drifted default must not be duplicated");
        let refreshed = after
            .get(&existing_id)
            .expect("drifted default keeps its id");
        assert_eq!(
            refreshed.prompt, spec.prompt,
            "prompt must be refreshed from the spec"
        );
        assert_eq!(
            refreshed.schedule,
            normalize_schedule(spec.schedule),
            "schedule must be refreshed from the spec"
        );
    });
}

#[test]
fn ensure_default_routines_logs_and_skips_on_write_failure() {
    // (d) write_routine failure branch: a regular FILE sits at the routine's directory path, so the
    // `create_dir_all` inside write_routine errors. The failure is logged and skipped, so an empty
    // store stays empty (the routine is never inserted).
    with_redirected_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();
        // Block every default's directory path with a regular file so create_dir_all fails for all.
        for spec in DEFAULT_ROUTINES {
            let slug = slugify(spec.title);
            std::fs::write(routines.join(&slug), "i am a file, not a dir").unwrap();
        }

        let store = empty_store();
        ensure_default_routines(&store);

        assert!(
            store.lock().unwrap().is_empty(),
            "a write failure must not insert the routine into the store"
        );
        // Every blocking path must still be a regular file (no write overwrote any of them).
        for spec in DEFAULT_ROUTINES {
            assert!(routines.join(slugify(spec.title)).is_file());
        }
    });
}

#[test]
fn is_default_slug_matches_only_built_ins() {
    let spec = &DEFAULT_ROUTINES[0];
    assert!(is_default_slug(&slugify(spec.title)));
    assert!(!is_default_slug("not-a-real-default"));
}

#[test]
fn tombstoned_default_is_not_reseeded() {
    // #265: a default absent from the store *because it was tombstoned* must stay absent, unlike
    // one that is merely never-seeded (covered by `ensure_default_routines_seeds_empty_store`).
    with_redirected_home(|_home| {
        let spec = &DEFAULT_ROUTINES[0];
        let slug = slugify(spec.title);
        record_removed_default(&slug);

        let store = empty_store();
        ensure_default_routines(&store);

        let after = store.lock().unwrap();
        assert!(
            !after
                .values()
                .any(|routine| slugify(&routine.title) == slug),
            "a tombstoned default must not be re-created on startup"
        );
    });
}

#[test]
fn tombstoning_one_default_does_not_suppress_the_others() {
    with_redirected_home(|_home| {
        let removed_spec = &DEFAULT_ROUTINES[0];
        record_removed_default(&slugify(removed_spec.title));

        let store = empty_store();
        ensure_default_routines(&store);

        let after = store.lock().unwrap();
        for spec in &DEFAULT_ROUTINES[1..] {
            let slug = slugify(spec.title);
            assert!(
                after
                    .values()
                    .any(|routine| slugify(&routine.title) == slug),
                "non-tombstoned default {:?} should still be seeded",
                spec.title
            );
        }
    });
}

#[test]
fn clearing_tombstone_lets_default_reseed() {
    with_redirected_home(|_home| {
        let spec = &DEFAULT_ROUTINES[0];
        let slug = slugify(spec.title);
        record_removed_default(&slug);
        clear_removed_default(&slug);

        let store = empty_store();
        ensure_default_routines(&store);

        let after = store.lock().unwrap();
        assert!(
            after
                .values()
                .any(|routine| slugify(&routine.title) == slug),
            "clearing the tombstone must let the default be re-seeded"
        );
    });
}

#[test]
fn record_removed_default_is_idempotent_and_persists_across_reads() {
    with_redirected_home(|_home| {
        let slug = "some-default";
        record_removed_default(slug);
        record_removed_default(slug);
        assert_eq!(read_removed_defaults().len(), 1);

        clear_removed_default(slug);
        assert!(read_removed_defaults().is_empty());
        // Clearing an already-cleared (or never-set) tombstone is a no-op, not an error.
        clear_removed_default(slug);
        assert!(read_removed_defaults().is_empty());
    });
}
