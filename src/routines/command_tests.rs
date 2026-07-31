#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::model::Routine;

/// Build a minimal routine for command-construction tests.
fn make_routine(title: &str) -> Routine {
    Routine {
        model: None,
        id: "cmd-test-id".to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do it".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
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

/// Run `body` with `PATH` set to `value`, restoring the previous value afterwards.
///
/// The test harness is single-threaded (`RUST_TEST_THREADS=1`), so mutating the
/// process-global `PATH` and restoring it around the call is safe.
fn with_path(value: &std::path::Path, body: impl FnOnce()) {
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; the value is restored immediately after.
    unsafe {
        std::env::set_var("PATH", value);
    }
    body();
    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(prev) => std::env::set_var("PATH", prev),
            None => std::env::remove_var("PATH"),
        }
    }
}

#[test]
fn build_routine_command_resolves_bin_dir_when_tool_on_path() {
    // Place a fake `tmux` executable in a temp dir and point PATH at it, so
    // `cron_path` -> `bin_dir("tmux")` actually *finds* the binary. This exercises the
    // `.find(..).map(str::to_string)` Some-resolution in `bin_dir` and the
    // `if let Some(dir) { dirs.push(dir) }` arm in `cron_path` — the path taken only when
    // a tool is present on PATH.
    let dir = std::env::temp_dir().join(format!("moadim-cmd-path-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tmux = dir.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let dir_str = dir.to_string_lossy().into_owned();
    with_path(&dir, || {
        let routine = make_routine("Cmd Path Routine");
        let agent = AgentCommand {
            command: "claude".to_string(),
            args: vec![],
            instructions_file: "CLAUDE.md".to_string(),
            setup: None,
        };
        let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
        // The resolved tmux dir is baked into the exported PATH.
        assert!(
            cmd.contains(&dir_str),
            "expected resolved tmux dir {dir_str} in: {cmd}"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_routine_command_extends_path_rather_than_replacing_it() {
    // The exported PATH must keep the login shell's `$PATH` (where version managers such as
    // nvm/pyenv/asdf/volta prepend their shim dirs when the profile is sourced) and only *append*
    // the curated fallback dirs. A bare `export PATH=<curated>` would drop those shims and silently
    // break agents that depend on a version-manager-selected node/python.
    let routine = make_routine("Path Extend Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(
        cmd.contains("export PATH=$PATH:"),
        "expected PATH to extend the profile's $PATH, not replace it, in: {cmd}"
    );
}

#[test]
fn build_routine_command_writes_daemon_preamble_before_prompt_copy() {
    // The daemon-managed instructions write into `$WB/CLAUDE.md` must fail-fast, mirroring the
    // `cp prompt.md` guard: a failed redirect (read-only/full $HOME, unwritable $WB, disk-quota)
    // must abort the launch before the prompt copy, setup, and tmux session — otherwise the agent
    // would run with no daemon-managed preamble.
    let routine = make_routine("Cmd Preamble Guard Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);

    // The primary write is guarded with an aborting `|| { ...; exit 1; }`.
    let write = cmd.find(r#"> "$WB/CLAUDE.md" || {"#).unwrap();
    assert!(
        cmd.contains(
            r#"> "$WB/CLAUDE.md" || { echo "moadim: failed to write agent instructions preamble; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }"#
        ),
        "expected the CLAUDE.md preamble write to fail-fast in: {cmd}"
    );

    // The guard must precede the prompt copy, so a failed preamble write never reaches it.
    let copy = cmd.find("/prompt.md\"").unwrap();
    assert!(
        write < copy,
        "preamble-write guard must precede the prompt copy"
    );

    // The best-effort user-prompt append stays best-effort (`|| true`), not aborting.
    assert!(
        cmd.contains(r#">> "$WB/CLAUDE.md" || true"#),
        "user-prompt append must remain best-effort in: {cmd}"
    );
}

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
#[path = "command_tests_part2.rs"]
mod command_tests_part2;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "command_tests_part3.rs"]
mod command_tests_part3;
