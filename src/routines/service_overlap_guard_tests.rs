#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;

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
    }
}

#[test]
fn svc_trigger_skips_spawn_when_a_previous_run_is_still_alive() {
    let _home = TempHome::set();
    // The overlap guard (#514): a tmux stub reporting a live session under this routine's
    // `moadim-{slug}-` prefix must suppress the new fire instead of launching a second, concurrent
    // agent session. The trigger still records its timestamp and returns Ok, the same non-fatal
    // shape as the other spawn-skip tests in `service_trigger_tests.rs`.
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let agent_name = "svc-trigger-overlap-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let title = "Svc Trigger Overlap ZZZ";
    let slug = slugify(title);
    let dir = std::env::temp_dir().join(format!("moadim-svc-overlap-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("tmux");
    // Real `$RID` shape (`SESS="moadim-$SLUG-$RID"`, `RID="${TS}_$$"`): digits, `_`, digits —
    // anything else no longer counts as a live fire of this routine (see `is_fire_of_prefix`).
    std::fs::write(
        &stub,
        format!("#!/bin/sh\nprintf 'moadim-{slug}-1730000000_4821\\n'\nexit 0\n"),
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let store = new_store();
    let mut routine = make_routine("trig-overlap-id", title, 1, 1);
    routine.agent = agent_name.into();
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-overlap-id".into(), routine);

    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
    unsafe { std::env::set_var("MOADIM_TMUX_BIN", &stub) };

    let triggered = svc_trigger(&store, "trig-overlap-id").unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
