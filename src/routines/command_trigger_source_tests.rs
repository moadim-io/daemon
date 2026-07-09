#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

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
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
fn build_routine_command_manual_omits_scheduled_trigger_stamp() {
    // A manual ("run now") trigger reuses the exact same launch script but must NOT append to
    // `scheduled.log` — otherwise an on-demand run would overwrite `last_scheduled_trigger_at`,
    // conflating manual and scheduled fires.
    let routine = make_routine("Cmd Manual No Stamp Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Manual);
    let log = crate::paths::routine_scheduled_log_path(&slugify(&routine.title))
        .to_string_lossy()
        .into_owned();
    assert!(
        !cmd.contains(&log),
        "manual trigger must not reference the scheduled-log path: {cmd}"
    );
    assert!(
        !cmd.contains(r#""$TS" >> "#),
        "manual trigger must not append the scheduled-trigger log: {cmd}"
    );
    // The agent still launches: the tmux session is still created.
    assert!(
        cmd.contains("tmux new-session"),
        "agent must still launch: {cmd}"
    );
}
