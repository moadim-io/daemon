#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);

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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(
        !cmd.contains("agent setup failed"),
        "did not expect a setup guard with no setup step in: {cmd}"
    );
}

#[test]
fn slugify_preserves_folder_segments() {
    assert_eq!(slugify("ops/nightly triage"), "ops/nightly-triage");
    assert_eq!(slugify("///ops///nightly triage///"), "ops/nightly-triage");
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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
