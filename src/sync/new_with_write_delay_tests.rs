
impl CronShim {
    fn new(initial: &str) -> Self {
        Self::new_with_write_delay(initial, 0)
    }

    /// Like [`Self::new`], but the shim sleeps `delay_ms` milliseconds between receiving a
    /// `crontab -` write on stdin and installing it, widening the window for concurrency
    /// regression tests that must observe two `sync_routines_to_crontab` calls overlapping (or
    /// not) in wall-clock time.
    fn new_with_write_delay(initial: &str, delay_ms: u32) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-rcronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        std::fs::write(&store_file, initial).unwrap();
        let store_display = store_file.to_string_lossy().into_owned();
        let script_path = base.join("crontab-shim.sh");
        let delay_secs = f64::from(delay_ms) / 1000.0;
        std::fs::write(
            &script_path,
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then cat \"$STORE\"; elif [ \"$1\" = \"-\" ]; then cat > \"$STORE.tmp\"; sleep {delay_secs}; mv \"$STORE.tmp\" \"$STORE\"; fi\n"
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

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "build_block_orders_tied_created_at_by_id_deterministically_tests.rs"]
mod build_block_orders_tied_created_at_by_id_deterministically_tests;
