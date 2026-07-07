#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn build_routine_command_uses_collision_resistant_run_id() {
    // Two runs of the same routine within the same wall-clock second must not collide on the
    // workbench dir or tmux session name. The run id mixes the launching shell's PID (`$$`, distinct
    // across concurrently-live processes) into `$TS`, and `$WB`/`$SESS` derive from that id. (#411)
    let expected_base = crate::paths::workbenches_dir()
        .to_string_lossy()
        .into_owned();
    let routine = make_routine("Cmd Run Id Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
        instructions_file: "CLAUDE.md".to_string(),
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);

    // The run id carries the PID for intra-second uniqueness (joined with `_` so the slug and
    // timestamp stay recoverable by `parse_workbench_name`).
    assert!(
        cmd.contains(r#"RID="${TS}_$$""#),
        "expected PID-mixed run id in: {cmd}"
    );
    // Workbench and session derive from the run id, not the bare second-granularity `$TS`.
    assert!(
        cmd.contains(&format!(
            r#"WB={}/"$SLUG-$RID""#,
            shell_quote(&expected_base)
        )),
        "workbench must use the run id: {cmd}"
    );
    assert!(
        cmd.contains(r#"SESS="moadim-$SLUG-$RID""#),
        "session name must use the run id: {cmd}"
    );
    assert!(
        !cmd.contains("$SLUG-$TS"),
        "workbench/session must not fall back to second-granularity `$TS`: {cmd}"
    );
}

#[test]
fn build_routine_command_fails_loudly_on_session_collision() {
    // If `tmux new-session` still fails (e.g. a residual session collision), the launch must abort
    // non-zero rather than silently no-op — mirroring the prompt-copy guard. (#411)
    let routine = make_routine("Cmd Session Guard Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
        instructions_file: "CLAUDE.md".to_string(),
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(
        cmd.contains(r#"tmux new-session -d -s "$SESS" -c "$WB""#)
            && cmd.contains(r#"aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }"#),
        "tmux new-session must abort loudly on failure: {cmd}"
    );
}
