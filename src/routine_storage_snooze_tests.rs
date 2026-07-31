#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Routine};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    }
}

/// A unique, not-yet-created scratch directory under the system temp dir.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-rs-{tag}-{}", uuid::Uuid::new_v4()))
}

/// Run `body` with `MOADIM_HOME_OVERRIDE` pointed at a fresh temp home, restoring the previous value
/// and removing the temp home afterwards. Mirrors the seam used by the agent registry tests.
fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_dir("override-home");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary; we set and immediately restore the
    // override around this call.
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

#[test]
fn repersist_routines_logs_on_write_failure() {
    // A routine whose slug directory path is occupied by a regular file makes write_routine fail,
    // exercising the `log::warn!` failure branch in repersist_routines.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-repersist-fail-id";
        let title = "Rs Repersist Fail Routine";
        let slug = slugify(title);
        // Block the slug dir with a regular file so create_dir_all fails inside write_routine.
        std::fs::write(routines.join(&slug), "block").unwrap();

        let mut map = HashMap::new();
        map.insert(id.to_string(), make_routine(id, title));
        let store = Arc::new(Mutex::new(map));
        repersist_routines(&store);

        // The write failed and was only logged; the blocking file remains.
        assert!(routines.join(&slug).is_file());
    });
}

// ─── New tests for previously uncovered lines ────────────────────────────────

#[test]
fn load_routine_from_dir_missing_title_returns_none() {
    // Covers L118: `let title = toml.title?;` — a TOML that has schedule and agent
    // but no `title` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-title-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "schedule = \"@daily\"\nagent = \"claude\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_from_dir_requires_schedule_cron_sidecar() {
    // A legacy routine.toml `schedule` is no longer a source of truth: the routine should load only
    // when the tracked schedule.cron sidecar exists.
    with_override_home(|_home| {
        let slug = "rs-no-schedule-cron-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs No Schedule Cron\"\nagent = \"claude\"\nschedule = \"@daily\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_from_dir_missing_agent_returns_none() {
    // Covers `agent: toml.agent?` — a TOML with title and schedule.cron but no `agent` field
    // causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-agent-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs No Agent\"\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@daily\n").unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[cfg(unix)]
#[test]
fn write_routine_fails_on_routine_toml_write_error() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers `atomic_write(&routine_toml_path(&slug), ..)?` — the dir is read-only so the atomic
    // write for `routine.toml` (which creates a sibling temp file) fails. The stale `run.sh`
    // removal before it is best-effort, so the read-only dir doesn't trip it.
    with_override_home(|_home| {
        let title = "Rs Toml Write Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = write_routine(&make_routine("rs-toml-write-fail-id", title));

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            result.is_err(),
            "write_routine should fail when routine.toml cannot be written"
        );
    });
}
include!("write_routine_fails_on_runtime_state_write_error_tests.rs");
