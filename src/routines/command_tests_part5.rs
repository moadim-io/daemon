
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
