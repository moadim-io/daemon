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
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn format_routine_line_invokes_script_with_schedule_and_tag() {
    let title = "Fid Sync Routine";
    let slug = slugify(title);
    let routine = make_routine("fid", title, "claude");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let line = format_routine_line(&routine, &agent).unwrap();
    assert!(line.starts_with("30 9 * * 1-5 "));
    // crontab line just runs the generated script — keeps it well under cron's length limit
    assert!(line.contains("/bin/sh "));
    // `-l` runs the script under a login shell so it sources the user's profile and the agent
    // inherits their environment (PATH, GH_TOKEN, …) instead of cron's minimal one.
    assert!(line.contains("/bin/sh -l "));
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
fn format_routine_line_creates_missing_parent_dir() {
    // Covers write_routine_script's create_dir_all(parent) branch: the routine's
    // directory does not exist yet, so the script write must create it first.
    let title = "Parent Create Sync Routine";
    let slug = slugify(title);
    // Ensure a clean slate so create_dir_all actually has work to do.
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
    assert!(!crate::paths::routine_dir(&slug).exists());

    let routine = make_routine("parent-create", title, "claude");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let line = format_routine_line(&routine, &agent).unwrap();
    assert!(line.ends_with("# moadim-routine:parent-create"));
    assert!(crate::paths::routine_script_path(&slug).exists());

    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}

#[test]
fn format_routine_line_returns_none_when_script_write_fails() {
    // Covers the Err(err) arm of format_routine_line: write_routine_script fails
    // because the routine directory path is occupied by a regular file, so
    // create_dir_all on it errors and the line is skipped (returns None).
    let title = "Write Fail Sync Routine";
    let slug = slugify(title);
    let routine_dir = crate::paths::routine_dir(&slug);
    let _ = std::fs::remove_dir_all(&routine_dir);
    if let Some(parent) = routine_dir.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    // Occupy the routine directory path with a regular file so that
    // create_dir_all(<routine_dir>) inside write_routine_script fails.
    std::fs::write(&routine_dir, "blocker").unwrap();

    let routine = make_routine("write-fail", title, "claude");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let result = format_routine_line(&routine, &agent);
    assert!(result.is_none(), "expected None on script write failure");

    let _ = std::fs::remove_file(&routine_dir);
}

#[test]
fn format_routine_line_when_parent_dir_already_exists() {
    // Covers write_routine_script's path where the routine directory (the script's parent) already
    // exists, so create_dir_all is a no-op success and execution proceeds to build + write the
    // script and format the line. Uses a keyword schedule and repositories to run the full body.
    let title = "Existing Parent Sync Routine";
    let slug = slugify(title);
    let routine_dir = crate::paths::routine_dir(&slug);
    // Pre-create the routine (parent) directory so create_dir_all has nothing to do.
    std::fs::create_dir_all(&routine_dir).unwrap();
    assert!(routine_dir.exists());

    let mut routine = make_routine("existing-parent", title, "claude");
    routine.schedule = "@daily".to_string();
    routine.repositories = vec![crate::routines::Repository {
        repository: "https://example.com/ctx.git".to_string(),
        branch: Some("main".to_string()),
    }];
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };

    let line = format_routine_line(&routine, &agent).unwrap();
    assert!(line.starts_with("@daily "), "wrong schedule: {line}");
    assert!(line.ends_with("# moadim-routine:existing-parent"));
    assert!(crate::paths::routine_script_path(&slug).exists());

    let _ = std::fs::remove_dir_all(&routine_dir);
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
