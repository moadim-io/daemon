#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn build_routine_command_sets_owner_only_umask_before_creating_files() {
    // The launch script sets `umask 077` before its first `mkdir`, so the workbench dir, the copied
    // `prompt.md`, and the tmux-piped `agent.log` are created owner-only rather than world-readable.
    let routine = make_routine("Cmd Umask Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    let umask_at = cmd.find("umask 077").expect("umask 077 statement present");
    let mkdir_at = cmd.find("mkdir -p").expect("workbench mkdir present");
    assert!(
        umask_at < mkdir_at,
        "umask must precede the first mkdir so the workbench tree is owner-only: {cmd}"
    );
}
