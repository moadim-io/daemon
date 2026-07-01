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
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
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

impl CronShim {
    fn new(initial: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-rcronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        std::fs::write(&store_file, initial).unwrap();
        let store_display = store_file.to_string_lossy().into_owned();
        let script_path = base.join("crontab-shim.sh");
        std::fs::write(
            &script_path,
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then cat \"$STORE\"; elif [ \"$1\" = \"-\" ]; then cat > \"$STORE\"; fi\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); restored on drop.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }
        Self {
            base,
            store_file,
            previous,
        }
    }

    fn store_contents(&self) -> String {
        std::fs::read_to_string(&self.store_file).unwrap_or_default()
    }
}

impl Drop for CronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test harness; restore the saved value.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn sync_refuses_to_wipe_routine_lines_when_store_is_empty() {
    // Footgun guard: an empty store must NOT overwrite a populated routines block — that would
    // silently drop every scheduled routine's cron line.
    let populated = format!(
        "{BLOCK_BEGIN}\n{BLOCK_HEADER}\n*/5 * * * * /bin/sh '/x/run.sh' # moadim-routine:keep-me\n{BLOCK_END}\n"
    );
    let shim = CronShim::new(&populated);
    sync_routines_to_crontab(&new_store()).unwrap();
    // The crontab is left untouched: the routine line survives.
    assert_eq!(shim.store_contents(), populated);
    assert!(shim.store_contents().contains("# moadim-routine:keep-me"));
}

#[test]
fn sync_proceeds_when_store_empty_but_no_routine_lines() {
    // Guard's second operand is false (no routine marker), so sync proceeds; the block is already
    // empty so the result equals the input and the idempotent check returns without writing.
    let empty_block = format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}\n");
    let shim = CronShim::new(&empty_block);
    sync_routines_to_crontab(&new_store()).unwrap();
    assert!(!shim.store_contents().contains("moadim-routine:"));
}

#[test]
fn sync_writes_block_for_a_loaded_store() {
    // A non-empty store with a resolvable agent passes the guard and writes its routine line.
    let agent_name = "test-sync-agent-write-block";
    let title = "Write Block Sync Routine";
    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let shim = CronShim::new("# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n");
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("w".into(), make_routine("w", title, agent_name));
    sync_routines_to_crontab(&store).unwrap();
    assert!(shim.store_contents().contains("# moadim-routine:w"));

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}

#[test]
fn build_block_orders_tied_created_at_by_id_deterministically() {
    // Two enabled managed routines sharing a created_at must emit in a stable, id-ordered
    // sequence regardless of HashMap iteration order, so the generated crontab block does not
    // churn across syncs. Insert in id-descending order to prove the sort — not insertion or
    // hash order — fixes the line order.
    let agent_name = "test-sync-agent-tied-order";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let title_a = "Tied Order Alpha Routine";
    let title_b = "Tied Order Beta Routine";
    let slug_a = slugify(title_a);
    let slug_b = slugify(title_b);

    let store = new_store();
    // id "b-tied" > "a-tied"; both created_at == 0 (the make_routine default).
    store
        .lock()
        .unwrap()
        .insert("b-tied".into(), make_routine("b-tied", title_b, agent_name));
    store
        .lock()
        .unwrap()
        .insert("a-tied".into(), make_routine("a-tied", title_a, agent_name));

    let block = build_block(&store);
    let pos_a = block.find("# moadim-routine:a-tied").unwrap();
    let pos_b = block.find("# moadim-routine:b-tied").unwrap();
    assert!(pos_a < pos_b, "lower id must sort first: {block}");
    // Stable across repeated builds.
    assert_eq!(block, build_block(&store));

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug_a));
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug_b));
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
    assert!(block.contains("schedule trigger 'inc'"));

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}

#[test]
fn build_block_excludes_routine_targeting_another_machine() {
    let agent_name = "test-sync-agent-other-machine";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    let mut routine = make_routine("other", "Other Machine Routine", agent_name);
    // Assigned to a machine that is not this host: it must not be scheduled here.
    routine.machines = vec!["definitely-not-this-host-zzz".to_string()];
    store.lock().unwrap().insert("other".into(), routine);
    let block = build_block(&store);
    assert!(!block.contains("moadim-routine:"));

    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn build_block_skips_routine_with_no_machine_assignment() {
    let store = new_store();
    // Empty `machines` means the routine runs nowhere — it is dormant and excluded (and logged as
    // such via `warn_dormant_routines`).
    let mut routine = make_routine("dormant", "Dormant Routine", "claude");
    routine.machines = vec![];
    store.lock().unwrap().insert("dormant".into(), routine);
    let block = build_block(&store);
    assert!(!block.contains("moadim-routine:"));
}

#[test]
fn build_block_empty_when_globally_locked() {
    let agent_name = "test-sync-agent-global-lock";
    let title = "Global Lock Sync Routine";
    let slug = crate::routines::slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    store.lock().unwrap().insert(
        "lock-test".into(),
        make_routine("lock-test", title, agent_name),
    );

    // Create the shared lock sentinel and verify it suppresses all crontab lines.
    let lock_path = crate::paths::global_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&lock_path, b"").unwrap();

    let block = build_block(&store);
    assert!(
        !block.contains("moadim-routine:"),
        "locked block must have no routine lines"
    );
    assert!(block.contains(BLOCK_BEGIN));
    assert!(block.contains(BLOCK_END));

    std::fs::remove_file(&lock_path).unwrap();
    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}
