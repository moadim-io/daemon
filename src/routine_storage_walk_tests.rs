use super::*;
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::routines::Routine;

fn scratch_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-walk-{}", uuid::Uuid::new_v4()))
}

fn with_home(body: impl FnOnce()) {
    let home = scratch_home();
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: test harness is single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body();
    // SAFETY: test harness is single-threaded.
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
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 1,
        updated_at: 2,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn walk_routines_recurses_into_nested_dirs() {
    with_home(|| {
        write_routine(&make_routine("walk-nested-id", "team/ops/nightly triage")).unwrap();
        let mut routines = HashMap::new();
        walk_routines(
            &crate::paths::routines_dir(),
            &crate::paths::routines_dir(),
            &mut routines,
            &|base, rel| super::super::load_routine_from_base(base, rel),
        );
        assert!(routines.contains_key("walk-nested-id"));
        assert!(crate::paths::routine_toml_path("team/ops/nightly-triage").exists());
        remove_routine_dir("team/ops/nightly-triage").unwrap();
    });
}

#[test]
fn walk_routines_skips_plain_dirs_without_routine_toml() {
    with_home(|| {
        std::fs::create_dir_all(crate::paths::routines_dir().join("archive/old")).unwrap();
        write_routine(&make_routine("walk-flat-id", "flat routine")).unwrap();
        let mut routines = HashMap::new();
        walk_routines(
            &crate::paths::routines_dir(),
            &crate::paths::routines_dir(),
            &mut routines,
            &|base, rel| super::super::load_routine_from_base(base, rel),
        );
        assert!(routines.contains_key("walk-flat-id"));
        assert_eq!(routines.len(), 1);
        remove_routine_dir("flat-routine").unwrap();
    });
}
