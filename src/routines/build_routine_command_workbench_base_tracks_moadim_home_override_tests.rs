
#[test]
fn build_routine_command_workbench_base_tracks_moadim_home_override() {
    // The `WB=` assignment must derive its base from `paths::workbenches_dir()` rather than a
    // hardcoded `$HOME/.moadim/workbenches` literal, so a run is launched under the same base the
    // reaper (`routines/cleanup/mod.rs`) and the LOGS view (`routines/service.rs`) scan. Exercise
    // this under `MOADIM_HOME_OVERRIDE` — a divergence here would leak workbenches the reaper never
    // sees and leave the LOGS view empty for real runs (see #601).
    let dir = std::env::temp_dir().join(format!("moadim-cmd-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: the test harness runs single-threaded; the prior value is restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }

    let expected_base = crate::paths::workbenches_dir()
        .to_string_lossy()
        .into_owned();
    let routine = make_routine("Cmd Workbench Base Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);

    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(prev) => std::env::set_var("MOADIM_HOME_OVERRIDE", prev),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        cmd.contains(&format!(
            r#"WB={}/"$SLUG-$RID""#,
            shell_quote(&expected_base)
        )),
        "expected WB base derived from paths::workbenches_dir() ({expected_base}) in: {cmd}"
    );
    assert!(
        !cmd.contains(r#"WB="$HOME/.moadim/workbenches"#),
        "expected the hardcoded $HOME/.moadim/workbenches literal to be gone, got: {cmd}"
    );
}

#[test]
fn cron_path_falls_back_to_root_home_when_home_unset() {
    // With HOME removed, `std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())` takes its
    // fallback arm, so the `~/.local/bin` etc. entries are anchored under `/root`.
    let saved = std::env::var_os("HOME");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("HOME");
    }

    let path = cron_path("definitely-not-a-real-binary-xyz");
    assert!(
        path.contains("/root/.local/bin"),
        "expected /root-anchored fallback dirs in: {path}"
    );

    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(prev) => std::env::set_var("HOME", prev),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[path = "command_run_id_tests.rs"]
mod command_run_id_tests;

#[path = "command_umask_tests.rs"]
mod command_umask_tests;

#[path = "command_trigger_source_tests.rs"]
mod command_trigger_source_tests;

#[path = "command_env_tests.rs"]
mod command_env_tests;

#[path = "command_repositories_tests.rs"]
mod command_repositories_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "build_routine_command_guards_agent_setup_step_tests.rs"]
mod build_routine_command_guards_agent_setup_step_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "inline_prompt_overflow_some_when_composed_prompt_exceeds_inline_limit_tests.rs"]
mod inline_prompt_overflow_some_when_composed_prompt_exceeds_inline_limit_tests;
