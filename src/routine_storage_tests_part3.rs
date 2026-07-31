#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn migrate_routine_dirs_moves_legacy_uuid_dir_to_slug() {
    with_override_home(|_home| {
        let id = "rs-legacy-uuid-1234";
        let title = "Rs Legacy Migrate Routine";
        let slug = slugify(title);
        let legacy_dir = crate::paths::routine_dir(id);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        // Legacy layout: routine.toml + prompt.md live under the UUID-named dir.
        let toml = format!(
            "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
        );
        std::fs::write(legacy_dir.join("routine.toml"), toml).unwrap();
        std::fs::write(legacy_dir.join("prompt.md"), "legacy prompt").unwrap();

        migrate_routine_dirs();

        // Legacy dir removed; canonical slug dir now holds toml + prompt sidecars, with the
        // legacy toml `prompt` field carried over into the new prompts/prompt.pure.md sidecar.
        assert!(!legacy_dir.exists(), "legacy UUID dir should be removed");
        assert!(crate::paths::routine_toml_path(&slug).exists());
        assert!(crate::paths::routine_cron_path(&slug).exists());
        assert!(crate::paths::routine_compiled_prompt_path(&slug).exists());
        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.id, id, "UUID id preserved across the dir migration");
        assert_eq!(loaded.prompt, "task");
    });
}
