#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{new_store, slugify, Routine};

fn make_routine(id: &str, title: &str, agent: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "30 9 * * 1-5".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: agent.to_string(),
        prompt: "p".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
    }
}

#[test]
fn format_routine_line_inlines_schedule_trigger_and_tag() {
    let title = "Fid Sync Routine";
    let slug = slugify(title);
    let routine = make_routine("fid", title, "claude");
    let line = format_routine_line(&routine);
    assert!(line.starts_with("30 9 * * 1-5 "));
    // The crontab line invokes the binary directly with the shell-quoted routine ID — no run.sh.
    assert!(line.contains("schedule trigger 'fid'"));
    assert!(line.ends_with("# moadim-routine:fid"));
    assert!(!line.contains('\n'));
    // No per-routine launch script is written any more.
    assert!(!crate::paths::routine_script_path(&slug).exists());
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}

#[test]
fn format_routine_line_honors_keyword_schedule() {
    // A `@`-keyword schedule is passed through `to_os_schedule` and prefixes the line.
    let routine = {
        let mut routine = make_routine("kw-id", "Keyword Sync Routine", "claude");
        routine.schedule = "@daily".to_string();
        routine
    };
    let line = format_routine_line(&routine);
    assert!(line.starts_with("@daily "), "wrong schedule: {line}");
    assert!(line.contains("schedule trigger 'kw-id'"));
    assert!(line.ends_with("# moadim-routine:kw-id"));
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
fn build_block_skips_routine_with_malformed_agent_config() {
    // A present-but-unparseable agent TOML must still be skipped, but for the *malformed* reason
    // (not the missing-file message). The routine never reaches the crontab block.
    let agent_name = "test-sync-agent-malformed-block";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    // `command` must be a string; an array makes the TOML structurally invalid for `AgentCommand`.
    std::fs::write(&cfg, "command = [\n").unwrap();

    let store = new_store();
    store.lock().unwrap().insert(
        "mal".into(),
        make_routine("mal", "Malformed Agent Sync Routine", agent_name),
    );
    let block = build_block(&store);
    assert!(!block.contains("moadim-routine:"));

    std::fs::remove_file(&cfg).unwrap();
}

/// A temp-dir `crontab` shim wired in via `MOADIM_CRONTAB_BIN`: `-l` prints the store file, `-`
/// overwrites it from stdin. Restores the prior env value and removes the temp dir on drop.
struct CronShim {
    base: std::path::PathBuf,
    store_file: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}
include!("new_with_write_delay_tests.rs");
