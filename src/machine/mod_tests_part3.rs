
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
