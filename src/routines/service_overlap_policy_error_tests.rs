#![allow(clippy::missing_docs_in_private_items, reason = "test fixtures")]

use super::*;

use crate::routines::new_store;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-overlap-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: this crate's tests serialize environment mutations.
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", &dir) };
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: this crate's tests serialize environment mutations.
        unsafe { std::env::remove_var("MOADIM_HOME_OVERRIDE") };
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn svc_trigger_fails_closed_when_overlap_policy_cannot_be_read() {
    let _home = TempHome::set();
    let agent_name = "overlap-policy-error-agent";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    std::fs::write(
        crate::paths::agent_toml_path(agent_name),
        "command = \"true\"\nargs = []\n",
    )
    .unwrap();
    let routine = Routine {
        id: "overlap-policy-error".into(),
        title: "Overlap Policy Error".into(),
        agent: agent_name.into(),
        schedule: "@daily".into(),
        schedules: vec![],
        prompt: "test".into(),
        enabled: true,
        source: "managed".into(),
        machines: vec![crate::machine::current_machine()],
        model: None,
        goal: None,
        repositories: vec![],
        disabled_reason: None,
        created_at: 1,
        updated_at: 1,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: Default::default(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
    };
    let rel_dir = crate::routine_storage::routine_rel_dir(&routine);
    let policy_path = crate::paths::routine_overlap_json_path(&rel_dir);
    std::fs::create_dir_all(policy_path.parent().unwrap()).unwrap();
    std::fs::write(&policy_path, "not json").unwrap();
    crate::routine_storage::write_routine(&routine).unwrap();

    let store = new_store();
    store.lock().unwrap().insert(routine.id.clone(), routine.clone());
    let triggered = svc_trigger(&store, &routine.id).unwrap();

    assert!(triggered.last_manual_trigger_at.is_some());
    let skip_log = std::fs::read_to_string(crate::paths::routine_skip_log_path(&rel_dir)).unwrap();
    assert!(skip_log.contains("invalid overlap policy"), "skip.log: {skip_log}");
}
