
#[test]
fn shell_quote_wraps_and_escapes() {
    assert_eq!(shell_quote("abc"), "'abc'");
    assert_eq!(shell_quote("a'b"), "'a'\\''b'");
}

#[test]
fn build_routine_command_contains_expected_pieces() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![
            "--dangerously-skip-permissions".to_string(),
            "{prompt}".to_string(),
        ],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(cmd.contains("tmux new-session -d -s \"$SESS\" -c \"$WB\""));
    // bakes a PATH export so cron's minimal PATH does not hide tmux/claude
    assert!(cmd.contains("export PATH="));
    // sanity-check: command must stay in a reasonable range; the PATH export and system-prompt
    // setup add several hundred chars, so the limit is higher than the raw cron-line minimum
    assert!(
        cmd.len() < 3000,
        "crontab line unexpectedly long: {} chars",
        cmd.len()
    );
    // prompt passed as a process argument via command substitution, no send-keys
    assert!(cmd.contains(r#""$(cat prompt.md)""#));
    assert!(!cmd.contains("send-keys"));
    assert!(!cmd.contains("capture-pane"));
    // pipe-pane is chained onto the same tmux invocation as new-session (#289), not a
    // standalone `tmux pipe-pane` statement.
    assert!(cmd.contains(r#"\; pipe-pane -o -t "$SESS""#));
    assert!(cmd.contains("SLUG='my-routine'"));
    // single line — no newlines
    assert!(!cmd.contains('\n'));
}

#[test]
fn build_routine_command_substitutes_arg_placeholders() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "codex".to_string(),
        args: vec!["exec".to_string(), "{prompt_file}".to_string()],
        instructions_file: "AGENTS.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    // The invocation is quoted as one `tmux new-session` shell-command argument together with
    // the exit-code capture appended to it (see `build_routine_command_records_exit_code_after_invocation`
    // in `command_tests.rs`), so the substituted invocation no longer stands alone as its own
    // quoted string.
    assert!(cmd.contains("codex exec prompt.md;"));
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "mod_tests_part2.rs"]
mod mod_tests_part2;
