#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

struct CronShim {
    base: std::path::PathBuf,
    store_file: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl CronShim {
    fn new(initial: Option<&str>) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-clearshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        if let Some(content) = initial {
            std::fs::write(&store_file, content).unwrap();
        }

        let store_display = store_file.to_string_lossy().into_owned();
        let script_body = format!(
            "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then\n  if [ -f \"$STORE\" ]; then cat \"$STORE\"; else echo \"no crontab for tester\" 1>&2; exit 1; fi\nelif [ \"$1\" = \"-\" ]; then\n  cat > \"$STORE\"\nfi\n"
        );
        let script_path = base.join("crontab-shim.sh");
        std::fs::write(&script_path, script_body).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }
        Self {
            base,
            store_file,
            previous,
        }
    }

    fn failing() -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-clearshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        let script_path = base.join("crontab-shim.sh");
        std::fs::write(
            &script_path,
            "#!/bin/sh\necho \"crontab boom\" 1>&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }
        Self {
            base,
            store_file,
            previous,
        }
    }

    fn write_fails(initial: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-clearshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        std::fs::write(&store_file, initial).unwrap();
        let store_display = store_file.to_string_lossy().into_owned();
        let script_body = format!(
            "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then\n  cat \"$STORE\"\nelif [ \"$1\" = \"-\" ]; then\n  cat > /dev/null\n  echo \"crontab write error\" 1>&2\n  exit 1\nfi\n"
        );
        let script_path = base.join("crontab-shim.sh");
        std::fs::write(&script_path, script_body).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
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
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

// ─── clear_managed_crontab_blocks (moadim uninstall, #380) ───────────────────

/// A user crontab line preceding the managed block.
const USER_BEFORE: &str = "0 0 * * * /usr/bin/backup";
/// A user crontab line following the managed block.
const USER_AFTER: &str = "0 9 * * * /usr/bin/report";

/// A crontab carrying the managed routines block wrapped by two unmanaged user
/// entries, mirroring a real install.
fn crontab_with_routines_block() -> String {
    format!(
        "{USER_BEFORE}\n\
         # BEGIN MOADIM-ROUTINES\n\
         # Managed by moadim — routines (agent tmux sessions)\n\
         */5 * * * * /bin/sh -l '/x/run.sh' # moadim-routine:r1\n\
         # END MOADIM-ROUTINES\n\
         {USER_AFTER}\n"
    )
}

#[test]
fn clear_removes_the_block_and_preserves_user_entries() {
    let shim = CronShim::new(Some(&crontab_with_routines_block()));
    let removed = clear_managed_crontab_blocks().unwrap();
    assert_eq!(removed, 1, "one routine line");
    let after = shim.store_contents();
    assert!(!after.contains("# BEGIN MOADIM-ROUTINES"));
    assert!(!after.contains("# moadim-routine:"));
    assert!(
        after.contains(USER_BEFORE),
        "user entry before the block survives"
    );
    assert!(
        after.contains(USER_AFTER),
        "user entry after the block survives"
    );
}

#[test]
fn clear_is_idempotent_on_an_already_clean_crontab() {
    let shim = CronShim::new(Some(&crontab_with_routines_block()));
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 1);
    let after_first = shim.store_contents();
    // A second uninstall has nothing managed to remove: returns 0 and leaves the
    // crontab byte-for-byte unchanged (no spurious rewrite).
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 0);
    assert_eq!(shim.store_contents(), after_first);
}

#[test]
fn clear_on_a_crontab_without_managed_blocks_is_a_noop() {
    let plain = format!("{USER_BEFORE}\n{USER_AFTER}\n");
    let shim = CronShim::new(Some(&plain));
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 0);
    assert_eq!(shim.store_contents(), plain, "untouched");
}
include!("clear_crontab_tests_part2.rs");
