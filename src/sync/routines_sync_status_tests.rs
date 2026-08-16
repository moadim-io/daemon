#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{new_store, Routine};

fn make_routine(id: &str, title: &str, agent: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "30 9 * * 1-5".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: agent.to_string(),
        prompt: "p".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
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

struct CronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl CronShim {
    fn write_fails(initial: &str) -> Self {
        Self::new(initial, true)
    }

    fn write_succeeds(initial: &str) -> Self {
        Self::new(initial, false)
    }

    fn new(initial: &str, fail_write: bool) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-rcronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        std::fs::write(&store_file, initial).unwrap();
        let store_display = store_file.to_string_lossy().into_owned();
        let script_path = base.join("crontab-shim.sh");
        let write_branch = if fail_write {
            "cat > /dev/null; echo \"write blocked\" 1>&2; exit 1"
        } else {
            "cat > \"$STORE\""
        };
        let script = format!(
            "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then\n  cat \"$STORE\"\nelif [ \"$1\" = \"-\" ]; then\n  {write_branch}\nfi\n"
        );
        std::fs::write(&script_path, script).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); restored on drop.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }
        Self { base, previous }
    }
}

impl Drop for CronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test harness; restore the saved value.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn failed_routine_sync_is_reported_in_crontab_sync_status() {
    crate::sync::reset_crontab_sync_status_for_tests();
    let agent_name = "test-sync-agent-status-failure";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let shim = CronShim::write_fails("# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n");
    let store = new_store();
    store.lock().unwrap().insert(
        "status-fail".into(),
        make_routine("status-fail", "Status Failure Sync Routine", agent_name),
    );

    let err = sync_routines_to_crontab(&store).unwrap_err();
    let status = crate::sync::crontab_sync_status();
    assert!(!status.ok);
    assert!(
        status
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains(&err.to_string())),
        "status should include sync error, got: {status:?}"
    );
    assert!(status.last_error_at.is_some());

    drop(shim);
    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn successful_routine_sync_clears_previous_crontab_sync_error() {
    failed_routine_sync_is_reported_in_crontab_sync_status();

    let shim = CronShim::write_succeeds("# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n");
    sync_routines_to_crontab(&new_store()).unwrap();
    let status = crate::sync::crontab_sync_status();
    assert!(status.ok);
    assert_eq!(status.last_error, None);
    assert_eq!(status.last_error_at, None);
    drop(shim);
}
