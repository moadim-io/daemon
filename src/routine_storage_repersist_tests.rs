//! `repersist_routines` tests, split out of `routine_storage_tests.rs` to keep that file under
//! the repo's line-count gate.

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Repository, Routine};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = std::env::temp_dir().join(format!("moadim-rs-repersist-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body(&home);
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
        }],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    }
}

#[test]
fn repersist_routines_recreates_missing_prompt_sidecar() {
    with_override_home(|_home| {
        let id = "rs-repersist-id";
        let title = "Rs Repersist Routine";
        let slug = slugify(title);
        write_routine(&make_routine(id, title)).unwrap();
        // Simulate the sync-only state: prompt.compiled.local.md and schedule.cron are gone.
        std::fs::remove_file(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
        std::fs::remove_file(crate::paths::routine_cron_path(&slug)).unwrap();
        assert!(!crate::paths::routine_compiled_prompt_path(&slug).exists());
        assert!(!crate::paths::routine_cron_path(&slug).exists());

        let mut map = HashMap::new();
        map.insert(id.to_string(), make_routine(id, title));
        let store = Arc::new(Mutex::new(map));
        repersist_routines(&store);

        assert!(
            crate::paths::routine_compiled_prompt_path(&slug).exists(),
            "repersist should recreate the prompt sidecar"
        );
        assert!(
            crate::paths::routine_cron_path(&slug).exists(),
            "repersist should recreate the cron sidecar"
        );
    });
}
