
#[test]
fn pid_file_write_read_clear_roundtrip() {
    let home = temp_home("pidfile");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    write_pid_file().unwrap();
    assert_eq!(read_pid_file(), Some(std::process::id()));
    let gitignore = crate::paths::config_gitignore_path();
    assert!(gitignore.exists());
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("*.local.*"),
        "gitignore must cover *.local.*"
    );
    assert!(
        content.contains("schedule.compailed.cron"),
        "gitignore must cover cron-union output"
    );
    assert!(
        content.contains("cache/"),
        "gitignore must cover repo cache"
    );
    assert!(
        content.contains(".compailed.cron"),
        "gitignore must keep covering legacy cron-union output"
    );
    // Manually remove one pattern; a second write must restore it without
    // duplicating the patterns already present.
    std::fs::write(&gitignore, "*.pid\n*.log\n").unwrap();
    write_pid_file().unwrap();
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("*.local.*"),
        "missing pattern must be re-added"
    );
    assert!(
        content.contains("schedule.compailed.cron"),
        "missing cron-union pattern must be re-added"
    );
    assert!(
        content.contains("cache/"),
        "missing repo cache pattern must be re-added"
    );
    assert!(
        content.contains(".compailed.cron"),
        "missing legacy cron-union pattern must be re-added"
    );
    assert_eq!(
        content.matches("*.pid").count(),
        1,
        "existing patterns must not duplicate"
    );
    // Write a file with all patterns but no trailing newline; the next write
    // must insert the newline separator before appending (line 495 branch).
    std::fs::write(&gitignore, "*.pid\n*.log").unwrap();
    write_pid_file().unwrap();
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("*.local.*"),
        "must append after no-trailing-newline content"
    );
    // All patterns present → early return (line 492 branch). Call twice; second is a no-op.
    write_pid_file().unwrap();
    assert_eq!(
        std::fs::read_to_string(&gitignore).unwrap(),
        content,
        "no-op write must not change file"
    );
    clear_pid_file();
    assert!(read_pid_file().is_none());
    // A garbage pid file parses to None rather than panicking.
    std::fs::write(crate::paths::pid_file(), "not-a-pid").unwrap();
    assert!(read_pid_file().is_none());
    // A pid file recording a dead process (u32::MAX is never a live PID on Unix) is reconciled
    // against liveness: reported as absent and cleaned up best-effort so it doesn't linger.
    std::fs::write(crate::paths::pid_file(), u32::MAX.to_string()).unwrap();
    assert!(read_pid_file().is_none());
    assert!(!crate::paths::pid_file().exists());
    // A pid file recording a live process (this test process) reads back unchanged.
    std::fs::write(crate::paths::pid_file(), std::process::id().to_string()).unwrap();
    assert_eq!(read_pid_file(), Some(std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn write_pid_file_seeds_readmes_without_clobbering_edits() {
    let home = temp_home("pidfile-readme");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    write_pid_file().unwrap();
    let config_readme = crate::paths::config_readme_path();
    let routines_readme = crate::paths::routines_readme_path();
    let agents_readme = crate::paths::agents_readme_path();
    assert!(config_readme.exists());
    assert!(routines_readme.exists());
    assert!(agents_readme.exists());
    assert!(std::fs::read_to_string(&config_readme)
        .unwrap()
        .contains("moadim config"));
    assert!(std::fs::read_to_string(&routines_readme)
        .unwrap()
        .contains("moadim routines"));
    assert!(std::fs::read_to_string(&agents_readme)
        .unwrap()
        .contains("moadim agents"));
    // A second start must not overwrite a user's edits to any of the READMEs.
    std::fs::write(&config_readme, "custom notes").unwrap();
    std::fs::write(&routines_readme, "custom notes").unwrap();
    std::fs::write(&agents_readme, "custom notes").unwrap();
    write_pid_file().unwrap();
    assert_eq!(
        std::fs::read_to_string(&config_readme).unwrap(),
        "custom notes"
    );
    assert_eq!(
        std::fs::read_to_string(&routines_readme).unwrap(),
        "custom notes"
    );
    assert_eq!(
        std::fs::read_to_string(&agents_readme).unwrap(),
        "custom notes"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn run_background_starts_when_none_running() {
    let home = temp_home("runbg-fresh");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    run_background().unwrap();
    let _ = std::fs::remove_dir_all(&home);
}

// ─── Additional coverage tests ────────────────────────────────────────────────

// Filesystem-blocked and timeout error-path tests for write_pid_file/spawn_detached/
// run_background/restart live in cli_spawn_error_tests.rs.

// `docs/moadim.1` hand-mirrors the CLI and hardcodes its own version in the `.TH` header
// (e.g. `"moadim 0.16.0"`). Nothing previously kept that in lockstep with `Cargo.toml`, so a
// release could silently ship a man page reporting the *previous* version (issue #556). Fail
// loudly on drift instead.

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "run_background_restarts_when_already_running_tests.rs"]
mod run_background_restarts_when_already_running_tests;
