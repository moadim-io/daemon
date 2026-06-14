#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{new_store, Routine};

fn make_routine(id: &str, agent: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "30 9 * * 1-5".to_string(),
        title: "Sync Routine".to_string(),
        agent: agent.to_string(),
        prompt: "p".to_string(),
        repositories: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
    }
}

#[test]
fn escape_percent_escapes_all() {
    assert_eq!(escape_percent("a%b%c"), "a\\%b\\%c");
    assert_eq!(escape_percent("no percent"), "no percent");
}

#[test]
fn format_routine_line_has_schedule_command_and_tag() {
    let r = make_routine("fid", "claude");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let line = format_routine_line(&r, &agent);
    assert!(line.starts_with("30 9 * * 1-5 "));
    assert!(line.contains("tmux new-session"));
    assert!(line.contains("date +\\%s")); // percent escaped for cron
    assert!(line.ends_with("# moadim-routine:fid"));
    assert!(!line.contains('\n'));
}

#[test]
fn build_block_empty_when_no_routines() {
    let block = build_block(&new_store());
    assert!(block.contains(BLOCK_BEGIN));
    assert!(block.contains(BLOCK_END));
    assert!(!block.contains("moadim-routine:"));
}

#[test]
fn build_block_skips_disabled_and_unmanaged() {
    let store = new_store();
    let mut disabled = make_routine("d", "no-cfg-agent-zzz");
    disabled.enabled = false;
    let mut system = make_routine("s", "no-cfg-agent-zzz");
    system.source = "system".to_string();
    store.lock().unwrap().insert("d".into(), disabled);
    store.lock().unwrap().insert("s".into(), system);
    let block = build_block(&store);
    assert!(!block.contains("moadim-routine:"));
}

#[test]
fn build_block_skips_routine_with_missing_agent_config() {
    let store = new_store();
    store.lock().unwrap().insert(
        "m".into(),
        make_routine("m", "definitely-missing-agent-zzz"),
    );
    let block = build_block(&store);
    // Missing agent config → routine skipped, block stays empty.
    assert!(!block.contains("moadim-routine:"));
}

#[test]
fn build_block_includes_routine_with_agent_config() {
    let agent_name = "test-sync-agent-build-block";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("inc".into(), make_routine("inc", agent_name));
    let block = build_block(&store);
    assert!(block.contains("# moadim-routine:inc"));
    assert!(block.contains("tmux new-session"));

    std::fs::remove_file(&cfg).unwrap();
}
