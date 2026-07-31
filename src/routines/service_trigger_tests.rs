#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;
use std::sync::Mutex;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
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

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
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
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    }
}

/// Serializes the tests that clear `PATH`, so concurrent service tests never see
/// a stripped environment. The poisoned-lock case is recovered into the guard.
static PATH_GUARD: Mutex<()> = Mutex::new(());

/// Run `body` with an empty `PATH`, restoring the original value afterwards.
fn with_empty_path(body: impl FnOnce()) {
    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    body();
    match saved {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
}

fn with_working_crontab(body: impl FnOnce()) {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("moadim-routcronok-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let script = base.join("crontab-ok.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let saved = std::env::var_os("MOADIM_CRONTAB_BIN");
    std::env::set_var("MOADIM_CRONTAB_BIN", &script);
    body();
    match saved {
        Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
        None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
    }
    let _ = std::fs::remove_dir_all(&base);
    drop(guard);
}

#[test]
fn svc_trigger_warns_when_spawn_fails() {
    let _home = TempHome::set();
    // With `PATH` cleared and an agent config present, `build_routine_command`
    // produces a command that `sh -c` cannot run because `sh` itself is not on
    // `PATH`, so the spawn fails and the warning branch runs. The trigger still
    // records its timestamp and returns.
    let agent_name = "svc-trigger-spawn-fail-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let title = "Svc Trigger Spawn Fail ZZZ";
    let store = new_store();
    let mut routine = make_routine("trig-spawn-id", title, 1, 1);
    routine.agent = agent_name.into();
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-spawn-id".into(), routine);

    with_empty_path(|| {
        let triggered = svc_trigger(&store, "trig-spawn-id").unwrap();
        assert!(triggered.last_manual_trigger_at.is_some());
    });
}

#[test]
fn svc_trigger_skips_spawn_when_prompt_exceeds_inline_limit() {
    let _home = TempHome::set();
    // An agent whose args inline `{prompt}`, combined with a composed prompt over the
    // inline-argument limit, must skip the spawn (#443) rather than launch a command doomed to
    // fail silently inside tmux with `E2BIG`. The trigger still records its timestamp and
    // returns Ok — the same non-fatal shape as `svc_trigger_warns_when_spawn_fails` above. `PATH`
    // is left as-is (unlike that test): the skip must happen before a spawn is ever attempted, not
    // because the shell can't be found.
    let agent_name = "svc-trigger-oversized-prompt-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = [\"{prompt}\"]\n").unwrap();

    let title = "Svc Trigger Oversized Prompt ZZZ";
    let store = new_store();
    let mut routine = make_routine("trig-oversized-id", title, 1, 1);
    routine.agent = agent_name.into();
    routine.prompt = "x".repeat(crate::routines::MAX_INLINE_PROMPT_BYTES * 2);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-oversized-id".into(), routine);

    let triggered = svc_trigger(&store, "trig-oversized-id").unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());
}
include!("svc_trigger_scheduled_missing_routine_not_found_tests.rs");
