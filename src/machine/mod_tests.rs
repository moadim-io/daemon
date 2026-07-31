//! Tests for machine identity resolution, persistence, CLI, and the targeting predicate.

use super::*;

/// Save an env var's prior value and restore it on drop, so a test's override never leaks. Tests in
/// this crate run single-threaded per binary (`RUST_TEST_THREADS=1`), so the global mutation is safe.
struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// Set `name` to `value`, remembering the prior value for restoration.
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
        unsafe { std::env::set_var(name, value) }
        Self { name, previous }
    }

    /// Ensure `name` is unset for the duration of the guard.
    fn unset(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
        unsafe { std::env::remove_var(name) }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

/// Create a unique tempdir to use as `MOADIM_HOME_OVERRIDE` for a test.
fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-machine-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

// ─── resolve_from precedence ───────────────────────────────────────────────

#[test]
fn resolve_from_prefers_env() {
    let (name, source) = resolve_from(
        Some("from-env".to_string()),
        Some("from-file".to_string()),
        "from-host".to_string(),
    );
    assert_eq!(name, "from-env");
    assert_eq!(source, MachineSource::Env);
}

#[test]
fn resolve_from_uses_file_when_no_env() {
    let (name, source) = resolve_from(None, Some("from-file".to_string()), "from-host".to_string());
    assert_eq!(name, "from-file");
    assert_eq!(source, MachineSource::File);
}

#[test]
fn resolve_from_falls_back_to_hostname() {
    let (name, source) = resolve_from(None, None, "from-host".to_string());
    assert_eq!(name, "from-host");
    assert_eq!(source, MachineSource::Hostname);
}

#[test]
fn resolve_from_treats_blank_env_and_file_as_absent() {
    // Whitespace-only env and file values must not win — they fall through to the hostname.
    let (name, source) = resolve_from(
        Some("   ".to_string()),
        Some("\t\n".to_string()),
        "from-host".to_string(),
    );
    assert_eq!(name, "from-host");
    assert_eq!(source, MachineSource::Hostname);
}

#[test]
fn resolve_from_trims_winning_value() {
    let (name, source) = resolve_from(Some("  padded  ".to_string()), None, "host".to_string());
    assert_eq!(name, "padded");
    assert_eq!(source, MachineSource::Env);
}

// ─── non_empty ─────────────────────────────────────────────────────────────

#[test]
fn non_empty_filters_blank_and_none() {
    assert_eq!(non_empty(None), None);
    assert_eq!(non_empty(Some("   ".to_string())), None);
    assert_eq!(non_empty(Some(" ok ".to_string())), Some("ok".to_string()));
}

// ─── hostname ──────────────────────────────────────────────────────────────

#[test]
fn hostname_is_non_empty() {
    assert!(!hostname().is_empty());
}

// ─── targets predicate ─────────────────────────────────────────────────────

#[test]
fn targets_matches_only_named_machine() {
    assert!(targets(&["a".to_string(), "b".to_string()], "b"));
    assert!(!targets(&["a".to_string()], "b"));
    // Empty list targets no machine.
    assert!(!targets(&[], "a"));
}

#[test]
fn targets_glob_entry_matches_any_or_a_family() {
    assert!(targets(&["*".to_string()], "anything"));
    let machines = vec!["box-*".to_string()];
    assert!(targets(&machines, "box-1"));
    assert!(!targets(&machines, "other-1"));
    // A glob is still a full match: no implicit substring/suffix.
    assert!(!targets(&machines, "prefix-box-1"));
}

// ─── glob_match ─────────────────────────────────────────────────────────────

#[test]
fn glob_match_without_star_is_exact() {
    assert!(glob_match("box", "box"));
    assert!(!glob_match("box", "boxes"));
    assert!(!glob_match("box", ""));
}

#[test]
fn glob_match_star_matches_prefix_middle_suffix_and_everything() {
    assert!(glob_match("*", "anything at all"));
    assert!(glob_match("box-*", "box-1"));
    // Name exhausted exactly at a trailing `*`: the post-loop "consume remaining stars" step runs.
    assert!(glob_match("box-*", "box-"));
    assert!(!glob_match("box-*", "other"));
    assert!(glob_match("*-work", "m4-work"));
    assert!(!glob_match("*-work", "m4-personal"));
    assert!(glob_match("a*z", "abcz"));
    assert!(!glob_match("a*z", "abcy"));
    assert!(glob_match("a**b", "axxxb")); // Consecutive stars collapse to one.
                                          // The first `*` in "*bc" greedily consumes too much and must backtrack for "bc" to land.
    assert!(glob_match("*bc", "abcbc"));
    assert!(!glob_match("*bc", "abcd"));
}

// ─── MachineSource labels ──────────────────────────────────────────────────

#[test]
fn source_labels_are_distinct() {
    assert_eq!(MachineSource::Env.label(), "MOADIM_MACHINE env");
    assert_eq!(MachineSource::File.label(), "machine.local.toml");
    assert_eq!(
        MachineSource::Generated.label(),
        "auto-generated (first run)"
    );
    assert_eq!(MachineSource::Hostname.label(), "system hostname");
}

// ─── file persistence + end-to-end resolution ──────────────────────────────

#[test]
fn read_machine_file_absent_is_none() {
    let home = temp_home("read-absent");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    assert_eq!(read_machine_file(), None);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn resolve_auto_generates_when_no_config() {
    let home = temp_home("auto-gen");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _env = EnvGuard::unset("MOADIM_MACHINE");

    // First call: no file exists → auto-generate and persist.
    let (name1, source1) = resolve();
    assert_eq!(source1, MachineSource::Generated);
    assert!(
        name1.starts_with("machine-") && name1.len() == "machine-".len() + 8,
        "generated name {name1:?} should match machine-{{8hex}}"
    );

    // File is now written: second call returns the same name from file.
    let (name2, source2) = resolve();
    assert_eq!(source2, MachineSource::File);
    assert_eq!(
        name2, name1,
        "second resolve should return the persisted name"
    );

    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn resolve_falls_back_to_hostname_when_write_fails() {
    let home = temp_home("write-fail");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _env = EnvGuard::unset("MOADIM_MACHINE");

    // Block set_machine() by placing a regular file where the config dir should be.
    // create_dir_all() will fail because it can't overwrite a file with a directory.
    let config_dir = home.join(".config").join("moadim");
    std::fs::create_dir_all(config_dir.parent().unwrap()).unwrap();
    std::fs::write(&config_dir, b"").unwrap(); // file, not a dir

    let (name, source) = resolve();
    assert_eq!(source, MachineSource::Hostname);
    assert!(!name.is_empty());

    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn read_machine_file_invalid_toml_returns_none() {
    let home = temp_home("read-invalid");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let config_dir = home.join(".config").join("moadim");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("machine.local.toml"),
        b"!!!not valid toml!!!",
    )
    .unwrap();
    // parse failure → None, not a panic.
    assert_eq!(read_machine_file(), None);
    let _ = std::fs::remove_dir_all(&home);
}

// ─── referenced_machines ───────────────────────────────────────────────────

// ─── CLI dispatch (run) ────────────────────────────────────────────────────

// `max_concurrent_runs_override` persistence tests (issue #1155) live in
// `mod_concurrency_tests.rs` (split out to keep this file under the line cap).

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "mod_tests_part2.rs"]
mod mod_tests_part2;
