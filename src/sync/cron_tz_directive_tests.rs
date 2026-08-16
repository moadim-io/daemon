#[test]
fn cron_tz_line_for_reflects_host_zone_when_no_override() {
    // The pre-#405 fallback: no per-routine override still emits the host's own resolvable zone
    // (or nothing, on a host where it can't be named), never leaving the previous routine's
    // directive to bleed into a routine with no override of its own.
    let routine = make_routine("no-tz", "No Timezone Override Routine", "claude");
    assert_eq!(
        cron_tz_line_for(&routine),
        crate::routines::local_timezone().map(|tz| format!("CRON_TZ={tz}"))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cron_tz_line_for_emits_the_override_on_linux() {
    let mut routine = make_routine("tz-override", "Timezone Override Routine", "claude");
    routine.timezone = Some("Asia/Jerusalem".to_string());
    assert_eq!(
        cron_tz_line_for(&routine),
        Some("CRON_TZ=Asia/Jerusalem".to_string())
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn cron_tz_line_for_drops_an_override_on_a_non_linux_host() {
    // `CRON_TZ` is a Linux-only mechanism; an override that somehow reached this host (e.g. a
    // routine.toml synced from a Linux machine sharing the config repo) must not be emitted where
    // BSD `cron` (macOS) would silently ignore it — fall back to the host's own zone instead,
    // exactly like the no-override case.
    let mut routine = make_routine("tz-override-non-linux", "Non-Linux Override Routine", "claude");
    routine.timezone = Some("Asia/Jerusalem".to_string());
    assert_eq!(
        cron_tz_line_for(&routine),
        crate::routines::local_timezone().map(|tz| format!("CRON_TZ={tz}"))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn build_block_scopes_cron_tz_per_routine_without_bleeding() {
    // Two enabled routines back to back, only the first with an override: the second must carry
    // its own `CRON_TZ` directive (the host's zone) rather than silently inheriting the first
    // routine's `Asia/Jerusalem` — crontab's `CRON_TZ` otherwise persists until reassigned.
    let agent_name = "test-sync-agent-cron-tz-scope";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let title_a = "Cron Tz Scope Alpha Routine";
    let title_b = "Cron Tz Scope Beta Routine";
    let slug_a = slugify(title_a);
    let slug_b = slugify(title_b);

    let store = new_store();
    let mut routine_a = make_routine("tz-scope-a", title_a, agent_name);
    routine_a.created_at = 0;
    routine_a.timezone = Some("Asia/Jerusalem".to_string());
    let mut routine_b = make_routine("tz-scope-b", title_b, agent_name);
    routine_b.created_at = 1;
    store.lock().unwrap().insert("tz-scope-a".into(), routine_a);
    store.lock().unwrap().insert("tz-scope-b".into(), routine_b);

    let block = build_block(&store);
    let directive_before_a = block
        .lines()
        .take_while(|line| !line.contains("# moadim-routine:tz-scope-a"))
        .last()
        .expect("routine a's line must be preceded by another line");
    assert_eq!(directive_before_a, "CRON_TZ=Asia/Jerusalem");

    let host_tz_directive = crate::routines::local_timezone().map(|tz| format!("CRON_TZ={tz}"));
    let directive_before_b = block
        .lines()
        .take_while(|line| !line.contains("# moadim-routine:tz-scope-b"))
        .last()
        .expect("routine b's line must be preceded by another line");
    if let Some(expected) = &host_tz_directive {
        assert_eq!(directive_before_b, expected);
    }
    // However the host resolves (or fails to), routine b must not have inherited routine a's
    // `Asia/Jerusalem` directive.
    assert_ne!(directive_before_b, "CRON_TZ=Asia/Jerusalem");

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug_a));
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug_b));
}
