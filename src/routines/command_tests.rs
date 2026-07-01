#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::model::Routine;

/// Build a minimal routine for command-construction tests.
fn make_routine(title: &str) -> Routine {
    Routine {
        id: "cmd-test-id".to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do it".to_string(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
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
        let cmd = build_routine_command(&routine, &agent);
        // The resolved tmux dir is baked into the exported PATH.
        assert!(
            cmd.contains(&dir_str),
            "expected resolved tmux dir {dir_str} in: {cmd}"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_routine_command_stamps_scheduled_trigger_sidecar() {
    // The generated launch script records each scheduled firing by writing `$TS` into the
    // routine's `scheduled.local.toml` sidecar (best-effort, before the prompt-copy guard), since
    // the OS crontab runs this script directly without the daemon observing the fire.
    let routine = make_routine("Cmd Scheduled Stamp Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    let sidecar = crate::paths::routine_scheduled_state_path(&slugify(&routine.title))
        .to_string_lossy()
        .into_owned();
    assert!(
        cmd.contains(&format!(
            r#"printf 'last_scheduled_trigger_at = %s\n' "$TS" > {} || true"#,
            shell_quote(&sidecar)
        )),
        "expected scheduled-trigger sidecar stamp in: {cmd}"
    );
    // It must run before the prompt-copy guard so an aborted run still records the firing.
    let stamp = cmd.find("last_scheduled_trigger_at").unwrap();
    let copy = cmd.find("/prompt.md\"").unwrap();
    assert!(stamp < copy, "sidecar stamp must precede the prompt copy");
}

#[test]
fn build_routine_command_fail_fasts_when_disclosure_write_fails() {
    // The routine-origin disclosure write into `$WB/CLAUDE.md` must fail-fast, mirroring the
    // `cp prompt.md` guard: a failed redirect (read-only/full $HOME, unwritable $WB, disk-quota)
    // must abort the launch before the prompt copy, setup, and tmux session — otherwise the agent
    // would run with no disclosure mandate.
    let routine = make_routine("Cmd Disclosure Guard Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);

    // The primary write is guarded with an aborting `|| { ...; exit 1; }`.
    let write = cmd.find(r#"> "$WB/CLAUDE.md" || {"#).unwrap();
    assert!(
        cmd.contains(
            r#"> "$WB/CLAUDE.md" || { echo "moadim: failed to write agent instructions disclosure; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }"#
        ),
        "expected the CLAUDE.md disclosure write to fail-fast in: {cmd}"
    );

    // The guard must precede the prompt copy, so a failed disclosure write never reaches it.
    let copy = cmd.find("/prompt.md\"").unwrap();
    assert!(
        write < copy,
        "disclosure-write guard must precede the prompt copy"
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
    let cmd = build_routine_command(&routine, &agent);

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
            r#"WB={}/"$SLUG-$TS""#,
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

    unsafe {
        match saved {
            Some(prev) => std::env::set_var("HOME", prev),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[test]
fn tmux_available_in_true_when_fake_tmux_present() {
    // A temp dir containing a fake `tmux` executable resolves as available — the "present" branch
    // of the injectable detection helper.
    let dir = std::env::temp_dir().join(format!("moadim-tmux-present-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tmux = dir.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    assert!(tmux_available_in(&dir.to_string_lossy()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_available_in_false_when_dir_has_no_tmux() {
    // A temp dir without a `tmux` file resolves as missing — the "missing" branch of the helper.
    let dir = std::env::temp_dir().join(format!("moadim-tmux-missing-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    assert!(!tmux_available_in(&dir.to_string_lossy()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_available_reads_live_path_present() {
    // `tmux_available()` reads the process `PATH`; pointed at a dir with a fake tmux it returns
    // true, exercising the `is_some_and(..)` Some/true arm.
    let dir = std::env::temp_dir().join(format!("moadim-tmux-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tmux = dir.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    with_path(&dir, || assert!(tmux_available()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_available_false_when_path_unset() {
    // With PATH removed entirely, `std::env::var("PATH").ok()` is None and `is_some_and` short-
    // circuits to false — the missing-PATH arm.
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("PATH");
    }
    assert!(!tmux_available());
    unsafe {
        if let Some(prev) = saved {
            std::env::set_var("PATH", prev);
        }
    }
}

#[test]
fn bin_dir_returns_none_when_path_unset() {
    // With PATH removed entirely, `std::env::var("PATH").ok()?` short-circuits to None.
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("PATH");
    }
    assert!(bin_dir("definitely-not-a-real-binary-xyz").is_none());
    unsafe {
        if let Some(prev) = saved {
            std::env::set_var("PATH", prev);
        }
    }
}
