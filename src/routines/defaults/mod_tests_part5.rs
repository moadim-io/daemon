
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

#[test]
fn record_removed_default_is_best_effort_on_write_failure() {
    // Documented as best-effort: a persist failure is logged, not propagated or panicked on.
    // Force `write_removed_defaults` to fail by putting a *directory* at the tombstone file's
    // path, so `std::fs::write` errors instead of succeeding.
    with_redirected_home(|_home| {
        let path = removed_default_routines_path();
        std::fs::create_dir_all(&path).unwrap();

        record_removed_default("some-default");
    });
}

#[test]
fn record_removed_default_is_best_effort_when_parent_dir_cannot_be_created() {
    // Same best-effort contract, but exercising the `create_dir_all(parent)` failure branch: put
    // a *file* at the tombstone's parent (config) dir path, so creating it as a directory fails.
    with_redirected_home(|_home| {
        let config_dir = crate::paths::config_dir();
        std::fs::create_dir_all(config_dir.parent().unwrap()).unwrap();
        std::fs::write(&config_dir, "not a dir").unwrap();

        record_removed_default("some-default");
    });
}
