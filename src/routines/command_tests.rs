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
    let cmd = build_routine_command(&routine, &agent);
    assert!(
        cmd.contains("export PATH=$PATH:"),
        "expected PATH to extend the profile's $PATH, not replace it, in: {cmd}"
    );
}

#[test]
fn build_routine_command_appends_scheduled_trigger_log() {
    // The generated launch script records each scheduled firing by appending `$TS` to the
    // routine's `scheduled.log` (best-effort, before the prompt-copy guard), since the OS crontab
    // runs this script directly without the daemon observing the fire.
    let routine = make_routine("Cmd Scheduled Stamp Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    let log = crate::paths::routine_scheduled_log_path(&slugify(&routine.title))
        .to_string_lossy()
        .into_owned();
    assert!(
        cmd.contains(&format!(
            r#"printf '%s\n' "$TS" >> {} || true"#,
            shell_quote(&log)
        )),
        "expected scheduled-trigger log append in: {cmd}"
    );
    // It must run before the prompt-copy guard so an aborted run still records the firing.
    let stamp = cmd.find("scheduled.log").unwrap();
    let copy = cmd.find("/prompt.md\"").unwrap();
    assert!(stamp < copy, "log append must precede the prompt copy");
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

    unsafe {
        match saved {
            Some(prev) => std::env::set_var("HOME", prev),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[test]
fn build_routine_command_guards_agent_setup_step() {
    // When an agent has a `setup` step, it must be fail-fast: a non-zero exit aborts the launch
    // before `tmux new-session` runs, mirroring the `cp prompt.md` guard. Otherwise a failed setup
    // (e.g. trust/onboarding pre-seed) is silently ignored and the agent hangs on the interactive
    // prompt until the watchdog reaps it.
    let routine = make_routine("Setup Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: Some("python3 seed.py".to_string()),
        instructions_file: "CLAUDE.md".to_string(),
    };
    let cmd = build_routine_command(&routine, &agent);

    // The setup is inserted verbatim, wrapped in a `{ ...; } || { ...; exit 1; }` guard...
    assert!(
        cmd.contains(
            r#"{ python3 seed.py; } || { echo "moadim: agent setup failed; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }"#
        ),
        "expected guarded setup step in: {cmd}"
    );
    // ...and the guard precedes the tmux launch, so a failed setup never reaches it.
    let setup_pos = cmd.find("agent setup failed").unwrap();
    let tmux_pos = cmd.find("tmux new-session").unwrap();
    assert!(
        setup_pos < tmux_pos,
        "setup guard must precede tmux new-session in: {cmd}"
    );
}

#[test]
fn build_routine_command_omits_setup_guard_when_no_setup() {
    // With no `setup` step the guard is absent entirely (the `if let Some(setup)` arm is skipped).
    let routine = make_routine("No Setup Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
        instructions_file: "CLAUDE.md".to_string(),
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(
        !cmd.contains("agent setup failed"),
        "did not expect a setup guard with no setup step in: {cmd}"
    );
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

#[test]
fn tmux_session_prefix_matches_the_sess_line_build_routine_command_emits() {
    // The overlap guard (#514) matches on `tmux_session_prefix(slug)` to find *any* live fire of a
    // routine, so the literal `TMUX_SESSION_PREFIX` it's built from must stay byte-for-byte in sync
    // with the `SESS=` line the launch script actually emits (`moadim-$SLUG-$RID`).
    let routine = make_routine("Cmd Session Prefix Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(
        cmd.contains(&format!(r#"SESS="{TMUX_SESSION_PREFIX}$SLUG-$RID""#)),
        "expected SESS line built from TMUX_SESSION_PREFIX in: {cmd}"
    );

    let slug = slugify(&routine.title);
    assert_eq!(
        tmux_session_prefix(&slug),
        format!("{TMUX_SESSION_PREFIX}{slug}-")
    );
}

#[test]
fn build_routine_command_records_exit_code_after_invocation() {
    // The tmux pane's shell-command must record `$?` to a *workbench-relative* `exit_code` file
    // (not `$WB/exit_code`: `$WB` is never exported, so the new shell tmux spawns wouldn't see it)
    // once the agent invocation finishes, so run-history can distinguish success from failure.
    let routine = make_routine("Cmd Exit Code Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["--permission-mode".to_string(), "auto".to_string()],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    // The whole tmux shell-command (invocation + exit-code capture) is itself shell-quoted as one
    // `tmux new-session` argument, which re-escapes the inner single quotes around `printf`'s
    // `'%s'` into `'\''` — assert on ordering/content of the unescaped pieces rather than the
    // exact (implementation-detail) escaped byte sequence.
    let new_session_pos = cmd.find("tmux new-session").unwrap();
    let invocation_pos = cmd.find("--permission-mode auto").unwrap();
    // Multiple `printf`s appear earlier in the script (the disclosure write, the scheduled-fire
    // stamp); only the one after the invocation is the exit-code capture.
    let printf_pos = invocation_pos + cmd[invocation_pos..].find("printf").unwrap();
    let exit_code_pos = cmd.rfind("> exit_code").unwrap();
    assert!(
        new_session_pos < invocation_pos
            && invocation_pos < printf_pos
            && printf_pos < exit_code_pos,
        "expected exit-code capture after the invocation inside tmux new-session in: {cmd}"
    );
    assert!(cmd.contains(r#""$?""#), "expected $? capture in: {cmd}");
    assert!(
        !cmd.contains("$WB/exit_code"),
        "exit_code must be workbench-relative, not $WB-prefixed, since $WB isn't exported: {cmd}"
    );
}

#[test]
fn build_routine_command_attaches_pipe_pane_in_the_same_tmux_invocation() {
    // `pipe-pane` must be chained onto the *same* tmux invocation as `new-session` via `\;`
    // (tmux's own multi-command separator) rather than run as a separate, later `;`-joined shell
    // statement — otherwise output the agent writes between session creation and that second
    // statement running is silently dropped from `agent.log` (#289).
    // Deliberately avoids "pipe" or "pane" in the title: it becomes part of the slugified
    // workbench/log paths embedded earlier in the script, which would otherwise collide with the
    // `pipe-pane` substring this test searches for.
    let routine = make_routine("Cmd Log Capture Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    let new_session_pos = cmd.find("tmux new-session").unwrap();
    // No `tmux pipe-pane` invocation as its own separate command: the only "pipe-pane" text is the
    // subcommand name chained after `new-session` via `\;` within the same `tmux` invocation.
    assert!(
        !cmd.contains("tmux pipe-pane"),
        "pipe-pane must not be a standalone tmux invocation, but chained onto new-session: {cmd}"
    );
    let pipe_pane_pos = cmd.find("pipe-pane").unwrap();
    let next_tmux_or_end = cmd[new_session_pos + 1..]
        .find("tmux ")
        .map_or(cmd.len(), |offset| new_session_pos + 1 + offset);
    assert!(
        new_session_pos < pipe_pane_pos && pipe_pane_pos < next_tmux_or_end,
        "expected pipe-pane chained via \\; inside the same tmux new-session invocation in: {cmd}"
    );
    assert!(
        cmd.contains(r#"\; pipe-pane -o -t "$SESS""#),
        "expected pipe-pane chained with tmux's own \\; separator, targeting $SESS, in: {cmd}"
    );
}

#[test]
fn inline_prompt_overflow_none_for_prompt_file_agent_regardless_of_size() {
    // `{prompt_file}` (codex/hermes) passes the prompt as a path, never as an inlined argument, so
    // it is never subject to the inline-argument cap no matter how large the composed prompt is.
    let mut routine = make_routine("Cmd Overflow Prompt File Routine");
    routine.prompt = "x".repeat(MAX_INLINE_PROMPT_BYTES * 2);
    let agent = AgentCommand {
        command: "codex".to_string(),
        args: vec!["exec".to_string(), "{prompt_file}".to_string()],
        instructions_file: "AGENTS.md".to_string(),
        setup: None,
    };
    assert_eq!(inline_prompt_overflow(&routine, &agent), None);
}

#[test]
fn inline_prompt_overflow_none_when_composed_prompt_fits() {
    let routine = make_routine("Cmd Overflow Small Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![
            "--permission-mode".to_string(),
            "auto".to_string(),
            "{prompt}".to_string(),
        ],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    assert_eq!(inline_prompt_overflow(&routine, &agent), None);
}

#[test]
fn inline_prompt_overflow_some_when_composed_prompt_exceeds_inline_limit() {
    // A `{prompt}` agent (the shipped `claude` default) with a composed prompt over the inline
    // cap must be flagged, so the caller can skip a launch doomed to fail silently (#443).
    let mut routine = make_routine("Cmd Overflow Large Routine");
    routine.prompt = "x".repeat(MAX_INLINE_PROMPT_BYTES * 2);
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![
            "--permission-mode".to_string(),
            "auto".to_string(),
            "{prompt}".to_string(),
        ],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let overflow = inline_prompt_overflow(&routine, &agent);
    assert_eq!(overflow, Some(compose_prompt(&routine).len()));
    assert!(overflow.unwrap() > MAX_INLINE_PROMPT_BYTES);
}

#[path = "command_run_id_tests.rs"]
mod command_run_id_tests;

#[path = "command_umask_tests.rs"]
mod command_umask_tests;
