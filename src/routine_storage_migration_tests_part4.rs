
#[test]
fn migrate_routine_dirs_from_dir_logs_on_write_failure() {
    // write_routine fails when a regular FILE occupies the slug directory path, so `create_dir_all`
    // for that slug errors. The function logs and continues, leaving the legacy dir untouched.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-write-fail-uuid";
        let title = "Rs Write Fail Routine";
        let slug = slugify(title);
        let legacy_dir = crate::paths::routine_dir(id);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let toml = format!(
            "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
        );
        std::fs::write(legacy_dir.join("routine.toml"), toml).unwrap();
        std::fs::write(legacy_dir.join("prompt.md"), "legacy").unwrap();

        // Place a regular file where the slug directory should go, so create_dir_all(&slug) fails.
        std::fs::write(routines.join(&slug), "i block the slug dir").unwrap();

        migrate_routine_dirs_from_dir(&routines);

        // The write failed, so the legacy dir is preserved and the slug path is still the file.
        assert!(legacy_dir.exists(), "legacy dir is left when write fails");
        assert!(routines.join(&slug).is_file());
    });
}

#[test]
fn migrate_routine_dirs_public_wrapper_runs() {
    // Exercises the public wrapper delegating into an empty override home.
    with_override_home(|_home| {
        migrate_routine_dirs();
    });
}
