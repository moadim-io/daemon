#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{new_store, slugify, Routine};

fn make_routine(id: &str, title: &str, agent: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "30 9 * * 1-5".to_string(),
        title: title.to_string(),
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
fn format_routine_line_invokes_script_with_schedule_and_tag() {
    let title = "Fid Sync Routine";
    let slug = slugify(title);
    let r = make_routine("fid", title, "claude");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let line = format_routine_line(&r, &agent).unwrap();
    assert!(line.starts_with("30 9 * * 1-5 "));
    // crontab line just runs the generated script — keeps it well under cron's length limit
    assert!(line.contains("/bin/sh "));
    assert!(line.contains(&format!("/{slug}/run.sh")));
    assert!(line.ends_with("# moadim-routine:fid"));
    assert!(!line.contains('\n'));
    // the long launch command lives in the script, not the crontab line
    let script = std::fs::read_to_string(crate::paths::routine_script_path(&slug)).unwrap();
    assert!(script.starts_with("#!/bin/sh\n"));
    assert!(script.contains("tmux new-session"));
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
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
    let mut disabled = make_routine("d", "Disabled Sync Routine", "no-cfg-agent-zzz");
    disabled.enabled = false;
    let mut system = make_routine("s", "System Sync Routine", "no-cfg-agent-zzz");
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
        make_routine(
            "m",
            "Missing Agent Sync Routine",
            "definitely-missing-agent-zzz",
        ),
    );
    let block = build_block(&store);
    // Missing agent config → routine skipped, block stays empty.
    assert!(!block.contains("moadim-routine:"));
}

#[test]
fn build_block_includes_routine_with_agent_config() {
    let agent_name = "test-sync-agent-build-block";
    let title = "Inc Sync Routine";
    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("inc".into(), make_routine("inc", title, agent_name));
    let block = build_block(&store);
    assert!(block.contains("# moadim-routine:inc"));
    assert!(block.contains(&format!("/{slug}/run.sh")));

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}
