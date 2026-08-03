
#[path = "routine_storage_load.rs"]
mod routine_storage_load;
use routine_storage_load::load_routine_from_dir;
pub use routine_storage_load::load_store;
#[cfg(test)]
pub(crate) use routine_storage_load::load_store_from_dir;
pub(crate) use routine_storage_load::reload_store_from_dir;

#[path = "routine_storage_location.rs"]
mod routine_storage_location;
pub(crate) use routine_storage_location::{
    folder_from_rel_dir, routine_rel_dir, routine_slug, slug_from_rel_dir,
};

#[path = "routine_storage_migrations.rs"]
mod routine_storage_migrations;
pub(crate) use routine_storage_migrations::{
    migrate_compiled_prompt_filename, migrate_prompt_files, migrate_prompts_to_subfolder,
    migrate_routine_dirs, migrate_trigger_logs,
};
#[cfg(test)]
pub(crate) use routine_storage_migrations::{
    migrate_compiled_prompt_filename_from_dir, migrate_prompt_files_from_dir,
    migrate_prompts_to_subfolder_from_dir, migrate_routine_dirs_from_dir,
    migrate_trigger_logs_from_dir,
};

#[cfg(test)]
#[path = "routine_storage_tests.rs"]
mod routine_storage_tests;

#[cfg(test)]
#[path = "routine_storage_repersist_tests.rs"]
mod routine_storage_repersist_tests;

#[cfg(test)]
#[path = "routine_storage_more_tests.rs"]
mod routine_storage_more_tests;

#[cfg(test)]
#[path = "routine_storage_prompt_sidecar_tests.rs"]
mod routine_storage_prompt_sidecar_tests;

#[cfg(test)]
#[path = "routine_storage_migration_tests.rs"]
mod routine_storage_migration_tests;

#[cfg(test)]
#[path = "routine_storage_schedule_cron_migration_tests.rs"]
mod routine_storage_schedule_cron_migration_tests;

#[cfg(test)]
#[path = "routine_storage_prompt_file_migration_tests.rs"]
mod routine_storage_prompt_file_migration_tests;

#[cfg(test)]
#[path = "routine_storage_compiled_prompt_migration_tests.rs"]
mod routine_storage_compiled_prompt_migration_tests;

#[cfg(test)]
#[path = "routine_storage_snooze_tests.rs"]
mod routine_storage_snooze_tests;

#[cfg(test)]
#[path = "routine_storage_trigger_log_migration_tests.rs"]
mod routine_storage_trigger_log_migration_tests;

#[cfg(test)]
#[path = "routine_storage_sidecar_state_tests.rs"]
mod routine_storage_sidecar_state_tests;

#[cfg(test)]
#[path = "routine_storage_slug_collision_tests.rs"]
mod routine_storage_slug_collision_tests;

#[cfg(test)]
#[path = "routine_storage_gitignore_tests.rs"]
mod routine_storage_gitignore_tests;

#[cfg(test)]
#[path = "routine_storage_disabled_reason_tests.rs"]
mod routine_storage_disabled_reason_tests;

#[cfg(test)]
#[path = "routine_storage_env_tests.rs"]
mod routine_storage_env_tests;

#[cfg(test)]
#[path = "routine_storage_location_tests.rs"]
mod routine_storage_location_tests;

#[cfg(test)]
#[path = "routine_storage_disabled_json_tests.rs"]
mod routine_storage_disabled_json_tests;
