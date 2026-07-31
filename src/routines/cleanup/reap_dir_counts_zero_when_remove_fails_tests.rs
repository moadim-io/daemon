
#[cfg(unix)]
#[test]
fn reap_dir_counts_zero_when_remove_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // An expired + dead workbench whose removal fails (parent dir is read-only) is
    // not counted, exercising the `Err` arm of the remove match.
    let base = std::env::temp_dir().join(format!(
        "moadim-cleanup-removefail-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "expired-100");
    std::fs::write(base.join("expired-100").join("inner"), b"x").unwrap();

    // Strip write permission from the parent so removing the child directory fails.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&base, perms).unwrap();

    let now = 1000;
    let ttl_for = |_slug: &str| 500_u64; // age 900 > 500 -> expired
    let dead = |_session: &str| false;
    let stats = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &dead,
        &noop_kill,
        &finish_at_trigger,
        &noop_persist,
    );
    // A read-only parent makes `remove_dir_all` fail for an unprivileged user, so
    // the directory survives and the Err arm runs (0 removed). Root bypasses the
    // permission check; tolerate that by only asserting consistency.
    if base.join("expired-100").exists() {
        assert_eq!(stats.removed, 0);
    }

    // Restore permissions so cleanup can proceed.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&base, perms).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

fn routine_with(schedule: &str, ttl_secs: Option<u64>) -> super::super::model::Routine {
    super::super::model::Routine {
        model: None,
        id: "x".into(),
        schedule: schedule.into(),
        schedules: vec![],
        title: "t".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    }
}

// ─── parse_workbench_name overflow (L55) ─────────────────────────────────────

// ─── cron_interval_secs second-fire None (L36) ───────────────────────────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "cleanup_expired_workbenches_scans_real_workbenches_dir_tests.rs"]
mod cleanup_expired_workbenches_scans_real_workbenches_dir_tests;
