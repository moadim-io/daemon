#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::Routine;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-persist-{}", uuid::Uuid::new_v4()));
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

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
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
        notifications: Default::default(),
        timezone: None,
    }
}

#[test]
fn persist_existing_routine_refuses_to_recreate_deleted_routine_toml() {
    let _home = TempHome::set();
    let routine = make_routine("persist-deleted-id", "Persist Deleted Routine");
    write_routine(&routine).unwrap();
    let rel_dir = routine_rel_dir(&routine);
    let toml = crate::paths::routine_toml_path(&rel_dir);
    std::fs::remove_file(&toml).unwrap();

    let err = persist_existing_routine(&routine).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(
        !toml.exists(),
        "routine.toml must not be recreated at {rel_dir}"
    );
}

#[test]
fn persist_existing_routine_rewrites_an_existing_routine_toml() {
    let _home = TempHome::set();
    let mut routine = make_routine("persist-existing-id", "Persist Existing Routine");
    write_routine(&routine).unwrap();
    routine.goal = Some("updated goal".to_string());

    persist_existing_routine(&routine).unwrap();

    let rel_dir = routine_rel_dir(&routine);
    assert!(crate::paths::routine_toml_path(&rel_dir).exists());
    let text = std::fs::read_to_string(crate::paths::routine_toml_path(&rel_dir)).unwrap();
    assert!(text.contains("updated goal"), "{text}");
}
