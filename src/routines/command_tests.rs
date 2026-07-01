#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::model::Routine;

/// Build a minimal routine for command-construction tests.
fn make_routine(title: &str) -> Routine {
    Routine {
        model: None,
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
fn resolve_tmux_bin_from_prefers_path_over_fallbacks() {
    // tmux present on `path` -> returned immediately, fallback_dirs never consulted (Some-arm of
    // `bin_dir_in`, early return before the fallback loop).
    let dir =
        std::env::temp_dir().join(format!("moadim-resolve-tmux-path-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tmux"), "#!/bin/sh\n").unwrap();

    let dir_str = dir.to_string_lossy().into_owned();
    let resolved = resolve_tmux_bin_from(&dir_str, &["/definitely/not/here".to_string()]);
    assert_eq!(resolved, format!("{dir_str}/tmux"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_tmux_bin_from_falls_back_to_first_matching_fallback_dir() {
    // Not on `path`, but present in the second fallback dir -> the `for` loop's `is_file()` Some
    // (true) arm returns from there, having skipped the first (missing) dir.
    let base =
        std::env::temp_dir().join(format!("moadim-resolve-tmux-fb-{}", uuid::Uuid::new_v4()));
    let missing = base.join("missing");
    let present = base.join("present");
    std::fs::create_dir_all(&present).unwrap();
    std::fs::write(present.join("tmux"), "#!/bin/sh\n").unwrap();

    let resolved = resolve_tmux_bin_from(
        "",
        &[
            missing.to_string_lossy().into_owned(),
            present.to_string_lossy().into_owned(),
        ],
    );
    assert_eq!(resolved, format!("{}/tmux", present.to_string_lossy()));

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn resolve_tmux_bin_from_returns_bare_name_when_nowhere_found() {
    // Neither `path` nor any fallback dir holds `tmux` -> the loop runs to completion and the
    // final bare `"tmux"` fallback is returned.
    let resolved = resolve_tmux_bin_from("", &["/definitely/not/here".to_string()]);
    assert_eq!(resolved, "tmux");
}

#[test]
fn tmux_fallback_dirs_are_anchored_under_home() {
    let dirs = tmux_fallback_dirs("/home/u");
    assert!(dirs.contains(&"/opt/homebrew/bin".to_string()));
    assert!(dirs.contains(&"/usr/local/bin".to_string()));
    assert!(dirs.contains(&"/home/u/.local/bin".to_string()));
}

#[test]
fn resolve_tmux_bin_reads_live_path_and_home() {
    // End-to-end live-env wrapper: with a fake tmux on PATH it resolves through the same
    // `bin_dir_in` Some-arm as `resolve_tmux_bin_from`, proving the live `PATH`/`HOME` plumbing
    // reaches it.
    let dir =
        std::env::temp_dir().join(format!("moadim-resolve-tmux-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tmux"), "#!/bin/sh\n").unwrap();

    let dir_str = dir.to_string_lossy().into_owned();
    with_path(&dir, || {
        assert_eq!(resolve_tmux_bin(), format!("{dir_str}/tmux"));
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_tmux_bin_falls_back_to_root_home_when_home_unset() {
    // With HOME removed, `std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())` takes its
    // fallback arm — mirrors `cron_path_falls_back_to_root_home_when_home_unset` for the identical
    // pattern here. `home` is computed unconditionally before the PATH/fallback-dir search, so this
    // covers the closure regardless of whether a real `tmux` is on the test machine's live PATH.
    let saved = std::env::var_os("HOME");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("HOME");
    }

    let _ = resolve_tmux_bin();

    unsafe {
        match saved {
            Some(prev) => std::env::set_var("HOME", prev),
            None => std::env::remove_var("HOME"),
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

#[test]
fn build_routine_command_appends_model_override() {
    // A routine-level model override is appended to the invocation as `--model <id>`, shell-quoted
    // to guard against the (user-controlled) model ID breaking out of the cron line.
    let mut routine = make_routine("Cmd Model Routine");
    routine.model = Some("claude-sonnet-4-6".to_string());
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["--permission-mode".to_string(), "auto".to_string()],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    // The whole invocation is itself shell-quoted once for the `tmux new-session` argument, which
    // re-escapes the inner `shell_quote(model)` quotes into `'\''`, so assert on ordering and
    // content rather than the exact (implementation-detail) escaped byte sequence.
    let args_pos = cmd.find("--permission-mode auto").unwrap();
    let model_pos = cmd.find("--model").unwrap();
    assert!(
        model_pos > args_pos,
        "expected --model after the agent's own args in: {cmd}"
    );
    assert!(
        cmd[model_pos..].contains("claude-sonnet-4-6"),
        "expected model id after --model in: {cmd}"
    );
}

#[test]
fn build_routine_command_omits_model_flag_when_unset() {
    // No routine-level model override means the invocation is unchanged from the agent's own args.
    let routine = make_routine("Cmd No Model Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(
        !cmd.contains("--model"),
        "expected no --model flag in: {cmd}"
    );
}
