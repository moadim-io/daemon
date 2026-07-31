#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
include!("mod_tests_part5.rs");
